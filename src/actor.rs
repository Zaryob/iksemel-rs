//! Send-compatible asynchronous facade: Rc DOM parsing stays on one worker thread.
use crate::{AsyncConnection, IksError, Result};
use std::collections::VecDeque;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
type Reply<T> = oneshot::Sender<Result<T>>;
enum Command {
    Start(Reply<()>),
    Send(String, Reply<()>),
    Receive(Reply<String>),
    Enable(bool, Option<u32>, Reply<crate::SmEnabled>),
    Reconnect(String, u16, Reply<()>),
    Close(Reply<()>),
}
#[derive(Clone)]
pub struct ActorConnection {
    commands: mpsc::Sender<Command>,
}
impl ActorConnection {
    pub async fn connect(
        host: &str,
        port: u16,
        domain: &str,
        timeout: Option<Duration>,
        capacity: usize,
    ) -> Result<Self> {
        let (commands, rx) = mpsc::channel(capacity.max(1));
        let (ready, started) = oneshot::channel();
        let host = host.to_owned();
        let domain = domain.to_owned();
        std::thread::Builder::new()
            .name("iksemel-xmpp".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready.send(Err(IksError::Io(error)));
                        return;
                    }
                };
                runtime.block_on(async move {
                    match AsyncConnection::connect(&host, port, &domain, timeout).await {
                        Ok(conn) => {
                            if ready.send(Ok(())).is_ok() {
                                run(conn, rx).await;
                            }
                        }
                        Err(error) => {
                            let _ = ready.send(Err(error));
                        }
                    }
                });
            })
            .map_err(IksError::Io)?;
        started.await.map_err(|_| IksError::NetDropped)??;
        Ok(Self { commands })
    }
    async fn submit<T>(
        &self,
        command: Command,
        response: oneshot::Receiver<Result<T>>,
    ) -> Result<T> {
        self.commands
            .send(command)
            .await
            .map_err(|_| IksError::NetDropped)?;
        response.await.map_err(|_| IksError::NetDropped)?
    }
    pub async fn start_stream(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.submit(Command::Start(tx), rx).await
    }
    pub async fn send_xml(&self, xml: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.submit(Command::Send(xml, tx), rx).await
    }
    /// Returns owned XML which is Send; parse it on the receiving task when needed.
    pub async fn recv_xml(&self) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.submit(Command::Receive(tx), rx).await
    }
    pub async fn enable_stream_management(
        &self,
        resume: bool,
        max: Option<u32>,
    ) -> Result<crate::SmEnabled> {
        let (tx, rx) = oneshot::channel();
        self.submit(Command::Enable(resume, max, tx), rx).await
    }
    pub async fn reconnect_and_resume(&self, host: &str, port: u16) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.submit(Command::Reconnect(host.into(), port, tx), rx)
            .await
    }
    pub async fn close(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.submit(Command::Close(tx), rx).await
    }
}
async fn run(mut conn: AsyncConnection, mut commands: mpsc::Receiver<Command>) {
    let mut receivers: VecDeque<Reply<String>> = VecDeque::new();
    let mut pending: VecDeque<String> = VecDeque::new();
    loop {
        while let Some(reply) = receivers.pop_front() {
            if reply.is_closed() {
                continue;
            }
            if let Some(xml) = pending.pop_front() {
                if let Err(Ok(xml)) = reply.send(Ok(xml)) {
                    pending.push_front(xml);
                }
            } else {
                receivers.push_front(reply);
                break;
            }
        }
        tokio::select! {
            command=commands.recv()=>match command {
                Some(Command::Start(reply))=>{let _=reply.send(conn.start_stream().await);},
                Some(Command::Send(xml,reply))=>{
                    let result=match crate::DomParser::parse_str(&xml){Ok(node)=>{let stanza=node.borrow().clone();conn.send_stanza(&stanza).await},Err(error)=>Err(error)};
                    let _=reply.send(result);
                },
                Some(Command::Receive(reply))=>receivers.push_back(reply),
                Some(Command::Enable(resume,max,reply))=>{let _=reply.send(conn.enable_stream_management(resume,max).await);},
                Some(Command::Reconnect(host,port,reply))=>{let _=reply.send(conn.reconnect_and_resume(&host,port).await);},
                Some(Command::Close(reply))=>{let _=reply.send(conn.close().await);break;},
                None=>{let _=conn.close().await;break;},
            },
            event=conn.recv_stanza(),if !receivers.is_empty()=>match event {
                Ok(node)=>pending.push_back(node.to_string()),
                Err(error)=>{if let Some(reply)=receivers.pop_front(){let _=reply.send(Err(error));}},
            }
        }
    }
}
