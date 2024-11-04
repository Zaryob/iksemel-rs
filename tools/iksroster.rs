use std::fs::File;
use std::io::{self, Read, Write};
use std::time::Duration;
use clap::{Parser, ValueEnum};
use iksemel::{Parser as IksParser, SaxHandler, IksError, Result, DomParser, IksNode};
use rpassword::prompt_password;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Download roster from server
    #[arg(short = 'b', long = "backup")]
    backup: Option<String>,

    /// Upload roster to server
    #[arg(short = 'r', long = "restore")]
    restore: Option<String>,

    /// Load/Save roster to file
    #[arg(short = 'f', long = "file")]
    file: Option<String>,

    /// Network timeout in seconds
    #[arg(short = 't', long = "timeout", default_value = "30")]
    timeout: u64,

    /// Use encrypted connection
    #[arg(short = 's', long = "secure")]
    secure: bool,

    /// Use SASL authentication
    #[arg(short = 'a', long = "sasl")]
    sasl: bool,

    /// Use plain text authentication
    #[arg(short = 'p', long = "plain")]
    plain: bool,

    /// Print exchanged XML data
    #[arg(short = 'l', long = "log")]
    log: bool,

    /// Input file path
    #[arg(short, long)]
    input: String,
    
    /// Output file path
    #[arg(short, long)]
    output: Option<String>,
}

struct Session {
    parser: IksParser<RosterHandler>,
    jid: String,
    password: String,
    features: u32,
    authorized: bool,
    counter: u64,
    set_roster: bool,
    job_done: bool,
    roster: Option<IksNode>,
}

impl Session {
    fn new(jid: &str, password: &str, set_roster: bool) -> Result<Self> {
        let handler = RosterHandler::new();
        Ok(Session {
            parser: IksParser::new(handler),
            jid: jid.to_string(),
            password: password.to_string(),
            features: 0,
            authorized: false,
            counter: 0,
            set_roster,
            job_done: false,
            roster: None,
        })
    }
}

struct RosterHandler {
    root: Option<Rc<RefCell<IksNode>>>,
    node_stack: Vec<Rc<RefCell<IksNode>>>,
}

impl RosterHandler {
    fn new() -> Self {
        RosterHandler {
            root: None,
            node_stack: Vec::new(),
        }
    }
}

impl SaxHandler for RosterHandler {
    fn on_tag(&mut self, name: &str, attributes: &[(String, String)], tag_type: iksemel::TagType) -> Result<()> {
        match tag_type {
            iksemel::TagType::Open | iksemel::TagType::Single => {
                let mut node = IksNode::new_tag(name);
                for (attr, value) in attributes {
                    node.add_attribute(attr, value);
                }
                let node_rc = Rc::new(RefCell::new(node));

                if let Some(parent_rc) = self.node_stack.last() {
                    let mut child = IksNode::new_tag(name);
                    for (attr, value) in attributes {
                        child.add_attribute(attr, value);
                    }
                    parent_rc.borrow_mut().add_child(child);
                    if tag_type == iksemel::TagType::Open {
                        self.node_stack.push(node_rc);
                    }
                } else {
                    self.root = Some(node_rc.clone());
                    if tag_type == iksemel::TagType::Open {
                        self.node_stack.push(node_rc);
                    }
                }
            },
            iksemel::TagType::Close => {
                if let Some(current) = self.node_stack.last() {
                    let current_ref = current.borrow();
                    let current_name = current_ref.find_attrib("name");
                    if current_name.map_or(false, |n| n == name) {
                        drop(current_ref);
                        self.node_stack.pop();
                    } else {
                        return Err(iksemel::IksError::BadXml);
                    }
                }
            },
        }
        Ok(())
    }
    
    fn on_cdata(&mut self, data: &str) -> Result<()> {
        if let Some(parent) = self.node_stack.last() {
            if !data.trim().is_empty() {
                let mut cdata = IksNode::new(iksemel::IksType::CData);
                cdata.set_content(data);
                parent.borrow_mut().add_child(cdata);
            }
        }
        Ok(())
    }
}

fn save_roster(file: &str, roster: &IksNode) -> Result<()> {
    let mut file = File::create(file)?;
    file.write_all(roster.to_string().as_bytes())?;
    Ok(())
}

fn load_roster(path: &str) -> Result<IksNode> {
    let contents = std::fs::read_to_string(path)?;
    let handler = RosterHandler::new();
    let mut parser = IksParser::new(handler);
    parser.parse(&contents)?;
    let handler = parser.handler();
    let root = handler.root.as_ref().unwrap().borrow().clone();
    Ok(root)
}

fn connect(_session: &mut Session) -> Result<()> {
    // TODO: Implement XMPP connection logic
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.backup.is_none() && args.restore.is_none() {
        eprintln!("What I'm supposed to do?");
        std::process::exit(1);
    }

    if args.restore.is_some() && args.backup.is_none() && args.file.is_none() {
        eprintln!("Store which roster?");
        std::process::exit(1);
    }

    let jid = args.backup.as_ref().or(args.restore.as_ref()).unwrap();
    let password = prompt_password(format!("Password for {}: ", jid)).unwrap();

    if let Some(backup_jid) = args.backup {
        let mut session = Session::new(&backup_jid, &password, false)?;
        connect(&mut session)?;

        if let Some(file) = args.file {
            if let Some(roster) = session.roster {
                save_roster(&file, &roster)?;
            }
        }
    } else if let Some(restore_jid) = args.restore {
        if let Some(file) = args.file {
            let roster = load_roster(&file)?;
            let mut session = Session::new(&restore_jid, &password, true)?;
            session.roster = Some(roster);
            connect(&mut session)?;
        }
    }

    let node = load_roster(&args.input)?;
    
    if let Some(output) = args.output {
        std::fs::write(output, node.to_string())?;
    } else {
        println!("{}", node.to_string());
    }

    Ok(())
} 