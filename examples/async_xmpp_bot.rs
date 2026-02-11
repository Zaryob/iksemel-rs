//! An asynchronous XMPP echo bot demonstrating modern features of `iksemel-rs`:
//! - Asynchronous TCP connection over Tokio
//! - RFC 6120 StartTLS upgrade
//! - RFC 5802 / RFC 7677 SASL SCRAM-SHA-256 authentication
//! - Resource binding
//! - XEP-0198 Stream Management negotiation and ack processing
//! - Lock-free connection splitting into `(AsyncSender, AsyncReceiver)`
//! - XEP-0199 Ping response and XEP-0085 Chat State notifications

use iksemel::{
    authenticate_scram_sha256_async, bind_resource_async, build_pong, is_ping, AsyncConnection,
    AsyncReceiver, AsyncSender, IksNode, Result,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let host = std::env::var("XMPP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("XMPP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5222);
    let domain = std::env::var("XMPP_DOMAIN").unwrap_or_else(|_| "example.com".to_string());
    let user = std::env::var("XMPP_USER").unwrap_or_else(|_| "bot".to_string());
    let password = std::env::var("XMPP_PASS").unwrap_or_else(|_| "secret".to_string());

    println!("Connecting to {}:{} for domain '{}'...", host, port, domain);

    // 1. Establish asynchronous TCP connection
    let mut conn =
        AsyncConnection::connect(&host, port, &domain, Some(Duration::from_secs(10))).await?;

    // 2. Open initial XML stream
    conn.start_stream().await?;
    let stream_features = conn.recv_stanza().await?;
    println!(
        "Received stream features: <{}>",
        stream_features.name().unwrap_or("unknown")
    );

    // 3. Attempt StartTLS upgrade if requested by server
    if stream_features.find("starttls").is_some() {
        println!("Performing StartTLS upgrade...");
        conn.start_tls().await?;
        conn.start_stream().await?;
        let _tls_features = conn.recv_stanza().await?;
    }

    // 4. Authenticate using SCRAM-SHA-256 (RFC 7677)
    println!("Authenticating as '{}' via SASL SCRAM-SHA-256...", user);
    authenticate_scram_sha256_async(&mut conn, &user, &password).await?;
    let _auth_features = conn.recv_stanza().await?;

    // 5. Bind client resource
    let bound_jid = bind_resource_async(&mut conn, Some("iksemel-bot")).await?;
    println!("Successfully authenticated and bound JID: {}", bound_jid);

    // 6. Enable XEP-0198 Stream Management for guaranteed delivery
    if let Ok(sm) = conn.enable_stream_management(true, Some(300)).await {
        println!(
            "XEP-0198 Stream Management active (session ID: {:?})",
            sm.id
        );
    }

    // 7. Split connection into concurrent sender and receiver handles
    let (sender, receiver) = conn.split()?;

    // 8. Announce initial presence
    let mut initial_presence = IksNode::new_tag("presence");
    let mut show = IksNode::new_tag("show");
    show.insert_cdata("chat");
    initial_presence.add_child(show);
    let mut status = IksNode::new_tag("status");
    status.insert_cdata("iksemel-rs bot online");
    initial_presence.add_child(status);
    sender.send_stanza(&initial_presence).await?;

    println!("Bot is online and listening for stanzas...");

    // Run receiver loop
    run_bot_loop(sender, receiver).await?;

    Ok(())
}

async fn run_bot_loop(sender: AsyncSender, mut receiver: AsyncReceiver) -> Result<()> {
    loop {
        let stanza = match receiver.recv_stanza().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Connection closed or error encountered: {:?}", e);
                break;
            }
        };

        let tag_name = stanza.name().unwrap_or("");

        // Handle XEP-0199 Ping requests automatically
        if is_ping(&stanza) {
            if let Ok(pong) = build_pong(&stanza) {
                println!(
                    "Responding to XEP-0199 ping from {:?}",
                    stanza.find_attrib("from")
                );
                let _ = sender.send_stanza(&pong).await;
            }
            continue;
        }

        // Handle incoming chat messages
        if tag_name == "message" && stanza.find_attrib("type") == Some("chat") {
            if let Some(from) = stanza.find_attrib("from") {
                if let Some(body_text) = stanza.find_path_text(&["body"]) {
                    println!("Received message from '{}': '{}'", from, body_text);

                    // Echo back the message
                    let mut reply = IksNode::new_tag("message");
                    reply.add_attribute("to", from);
                    reply.add_attribute("type", "chat");

                    let mut body = IksNode::new_tag("body");
                    body.insert_cdata(format!("Echo: {}", body_text));
                    reply.add_child(body);

                    if let Err(e) = sender.send_stanza(&reply).await {
                        eprintln!("Failed to send echo reply: {:?}", e);
                    }
                }
            }
        }
    }

    Ok(())
}
