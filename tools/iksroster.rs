use clap::Parser;
use iksemel::{
    authenticate_non_sasl, authenticate_plain, bind_resource, establish_session, fetch_roster,
    sync_roster, Connection, IksError, Jid, Result, Roster,
};
use rpassword::prompt_password;
use std::io::{self, Read};
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "iksroster",
    author = "Süleyman Poyraz, Gurer Ozen",
    version = "0.2.0",
    about = "XMPP roster backup and restore utility"
)]
struct Args {
    /// Download roster from server for specified JID
    #[arg(short = 'b', long = "backup", value_name = "JID")]
    backup: Option<String>,

    /// Upload roster to server for specified JID
    #[arg(short = 'r', long = "restore", value_name = "JID")]
    restore: Option<String>,

    /// Load or save roster to file (defaults to stdin/stdout)
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    file: Option<String>,

    /// Server host (defaults to domain part of JID)
    #[arg(short = 'H', long = "host", value_name = "HOST")]
    host: Option<String>,

    /// Server port (defaults to 5222)
    #[arg(short = 'P', long = "port", default_value = "5222")]
    port: u16,

    /// Network timeout in seconds
    #[arg(short = 't', long = "timeout", default_value = "30")]
    timeout: u64,

    /// Use StartTLS encrypted connection
    #[arg(short = 's', long = "secure")]
    secure: bool,

    /// Force SASL authentication
    #[arg(short = 'a', long = "sasl")]
    sasl: bool,

    /// Use legacy Non-SASL plain text authentication
    #[arg(short = 'p', long = "plain")]
    plain: bool,

    /// Print exchanged XML traffic
    #[arg(short = 'l', long = "log")]
    log: bool,
}

fn connect_and_login(jid: &Jid, password: &str, args: &Args) -> Result<Connection> {
    let domain = jid.domain();
    let host = args.host.as_deref().unwrap_or(domain);
    let timeout = Some(Duration::from_secs(args.timeout));

    let mut conn = Connection::connect(host, args.port, domain, timeout)?;
    conn.set_log_traffic(args.log);

    // Initial stream start
    conn.start_stream()?;

    // Receive initial stream features
    let _features = conn.recv_stanza()?;

    // Negotiate StartTLS if requested
    if args.secure {
        conn.set_allow_insecure_tls(true);
        conn.start_tls()?;
        // Re-receive features after TLS restart
        let _ = conn.recv_stanza()?;
    }

    let node = jid.node().unwrap_or("");
    let resource = jid.resource().unwrap_or("iksroster");

    if args.plain {
        // Legacy Non-SASL authentication
        authenticate_non_sasl(&mut conn, node, password, resource, None)?;
    } else {
        // SASL PLAIN authentication (RFC 6120)
        authenticate_plain(&mut conn, node, password, None)?;
        // Receive features after SASL stream restart
        let _ = conn.recv_stanza()?;

        // Bind resource and establish session
        let _bound_jid = bind_resource(&mut conn, Some(resource))?;
        let _ = establish_session(&mut conn);
    }

    Ok(conn)
}

fn handle_backup(jid_str: &str, args: &Args) -> Result<()> {
    let jid = Jid::new(jid_str)?;
    let password =
        prompt_password(format!("Password for {}: ", jid.bare())).map_err(IksError::Io)?;

    let mut conn = connect_and_login(&jid, &password, args)?;
    let roster = fetch_roster(&mut conn, "roster_get_1")?;
    conn.close()?;

    if let Some(ref path) = args.file {
        roster.save_to_file(path)?;
        println!("Roster saved successfully to '{}'.", path);
    } else {
        println!("{}", roster.to_node());
    }

    Ok(())
}

fn handle_restore(jid_str: &str, args: &Args) -> Result<()> {
    let jid = Jid::new(jid_str)?;

    let roster = if let Some(ref path) = args.file {
        Roster::load_from_file(path)?
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        let doc = iksemel::DomParser::parse_str(&buffer)?;
        Roster::from_node_ref(&doc)?
    };

    let password =
        prompt_password(format!("Password for {}: ", jid.bare())).map_err(IksError::Io)?;

    let mut conn = connect_and_login(&jid, &password, args)?;
    sync_roster(&mut conn, &roster)?;
    conn.close()?;

    println!(
        "Synchronized {} roster contact(s) to server.",
        roster.items.len()
    );
    Ok(())
}

fn main() {
    let args = Args::parse();

    if args.backup.is_none() && args.restore.is_none() {
        eprintln!("Error: specify either --backup <JID> or --restore <JID>.");
        std::process::exit(1);
    }

    let result = if let Some(ref backup_jid) = args.backup {
        handle_backup(backup_jid, &args)
    } else if let Some(ref restore_jid) = args.restore {
        handle_restore(restore_jid, &args)
    } else {
        Ok(())
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
