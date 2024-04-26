use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::TcpStream;
use std::io::{Write, BufReader, BufRead};
use std::thread::{sleep, self};
use std::time::Duration;
use std::sync::mpsc::{self, Receiver, Sender};

const SLEEP_DURATION: Duration = Duration::from_millis(50);

// Escape sequences
const CLEAR: &str = "\x1B[2J";
const RESET_CURSOR: &str = "\x1B[1;1H";
const UP_ONE_LINE: &str = "\x1B[1F";

fn main() {
    // Get address
    print!("{}{}", CLEAR, RESET_CURSOR);
    println!("Enter server address:");
    let mut address = String::new();
    std::io::stdin().read_line(&mut address).expect("Failed reading user input");
    address = address.trim().to_owned();

    // Attempt connection
    let stream = match TcpStream::connect(&address) {
        Ok(stream) => {
            println!("Connected to server at {}", address);
            stream
        },
        Err(_) => {
            println!("Could not connect to server at address: {address}");
            println!("Shutting down...");
            std::process::exit(1);
        }
    };

    // Get username
    println!("Enter username:");
    let mut username = String::new();
    std::io::stdin().read_line(&mut username).expect("Failed reading user input");
    let username = username.trim().to_owned();
    print!("{}{}", CLEAR, RESET_CURSOR);

    println!("Enter password:");
    let mut password = String::new();
    std::io::stdin().read_line(&mut password).expect("Failed reading user input");
    let password = password.trim().to_owned();
    print!("{}{}", CLEAR, RESET_CURSOR);

    let mut channel = "general".to_owned();


    let (sender, receiver) = mpsc::channel::<Message>();
    let write_stream = stream.try_clone().expect("Failed copying TCP stream");
    let reader = BufReader::new(stream);

    let (token_sender, token_receiver) = mpsc::channel::<String>();

    // Create threads for communicating with server
    thread::spawn(move|| listen(reader, token_sender));
    thread::spawn(move|| write_queued(write_stream, receiver));

    let mut body = String::new();

    let auth = Message::Auth(AuthMessage {
        username: username.clone(),
        auth_token: None,
        password: Some(password)
    });

    sender.send(auth).expect("MPSC error");
    let auth_token = token_receiver.recv().unwrap();

    loop {
        body.clear();
        std::io::stdin().read_line(&mut body).expect("Failed reading user input");
        print!("{}", UP_ONE_LINE);
        // Quit client if user types "q"
        let outgoing_message: Message = match body.trim() {
            "q" => {
                println!("Quitting...");
                std::process::exit(0);
            },
            "h" => Message::Command(CommandMessage{
                username: username.clone(),
                auth_token: auth_token.clone(),
                command_type: "history".to_owned(),
                args: vec![channel.clone(), "50".to_owned()]
            }),
            "c" => {
                println!("Enter new channel:");
                channel.clear();
                std::io::stdin().read_line(&mut channel).expect("Failed reading user input");
                channel = channel.trim().to_owned();
                continue;
            },
            "d" => {
                println!("Enter users to DM:");
                channel.clear();
                let mut user_input = String::new();
                std::io::stdin().read_line(&mut user_input).expect("Failed reading user input");
                channel = generate_dm_name(&mut user_input.split(" ").collect::<Vec<_>>());
                continue;
            },
            _ => Message::Text(TextMessage 
            {
                username: username.clone(),
                body: body.clone(),
                channel: channel.clone(),
                auth_token: auth_token.clone(),
                embed_pointer: None,
                embed_type: None,
                message_id: None,
                timestamp: 0,
            })
        };

        sender.send(outgoing_message).expect("MPSC error");
    }
}

// Listen for messages from server and print them
fn listen(mut reader: BufReader<TcpStream>, token_sender: Sender<String>) {
    let mut incoming_message = String::new();
    // Listen for new messages and handle them
    loop {
        sleep(SLEEP_DURATION);
        // Clear possible old message
        incoming_message.clear();
        // Read incoming message and close stream if client has disconnected
        match reader.read_line(&mut incoming_message) {
            Ok(0) => {
                println!("Server disconnected, quitting...");
                std::process::exit(1);
            },
            Ok(_) => {}
            Err(_) => {
                println!("Server disconnected, quitting...");
                std::process::exit(1);
            },
        };
        // If no message has appeared, keep waiting
        if incoming_message.is_empty() {
            continue;
        }
        let message = from_str::<Message>(&incoming_message).unwrap();
        match message {
            Message::Text(inner) => {
                print!("<{username}> {body}", username = inner.username, body = inner.body);
            },
            Message::Auth(inner) => {
                println!("weee");
                token_sender.send(inner.auth_token.unwrap()).unwrap();
            },
            _ => {}
        }
    }   
}

// Write any queued messages to the server
fn write_queued(mut stream: TcpStream, receiver: Receiver<Message>) {
    loop {
        if let Ok(outgoing_message) = receiver.try_recv() {
            match stream.write((to_string(&outgoing_message).unwrap() + "\n").as_bytes()) {
                Ok(0) => {
                    println!("Server disconnected, quitting...");
                    std::process::exit(1);
                },
                Ok(_) => {}
                Err(_) => {
                    println!("Server disconnected, quitting...");
                    std::process::exit(1);
                },
            }
        }
        sleep(SLEEP_DURATION);
    }
}

fn generate_dm_name(usernames: &mut [&str]) -> String {
    let mut hashes = Vec::new();
    let mut ret = "DM".to_owned();
    for user in usernames {
        let mut hasher = DefaultHasher::new();
        user.trim().hash(&mut hasher);
        let val = (hasher.finish() >> 1) as i64;
        hashes.push(val);
    }
    hashes.sort();
    for hash in hashes {
        ret += "_";
        ret += &hash.to_string();
    }
    println!("{}", ret);
    ret
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Message {
    Text(TextMessage),
    File(FileMessage),
    Command(CommandMessage),
    Auth(AuthMessage)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextMessage {
    pub username: String,
    pub auth_token: String,
    pub body: String,
    pub channel: String,
    pub embed_pointer: Option<usize>,
    pub embed_type: Option<String>,
    pub message_id: Option<u32>,
    pub timestamp: u64
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileMessage {
    pub username: String,
    pub auth_token: String,
    pub filename: String,
    pub data: String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommandMessage {
    pub username: String,
    pub auth_token: String,
    pub command_type: String,
    pub args: Vec<String>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthMessage {
    pub username: String,
    pub auth_token: Option<String>,
    pub password: Option<String>
}