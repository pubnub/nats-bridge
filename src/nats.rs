use crate::socket::Socket;

pub struct Message {
    pub root: String,
    pub subject: String,
    pub my_id: String,
    pub sender_id: String,
    pub data: String,
}

#[derive(Debug)]
pub enum Error {
    Initialize,
    Publish,
    Subscribe,
    Ping,
    Exit,
}

pub struct SubscribeClient {
    socket: Socket,
    client_id: String,
    root: String,
    subject: String,
}

pub struct PublishClient {
    socket: Socket,
    root: String,
}

// =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
/// # NATS Subscribe Client
///
/// This client lib offers subscribe support to NATS.
///
/// ```no_run
/// use nats_bridge::nats::SubscribeClient;
///
/// let root = "subjects"; // subjects.demo
/// let subject = "demo";  // subjects.demo
/// let mut nats = SubscribeClient::new("0.0.0.0:4222", root, subject)
///     .expect("NATS Subscribe Client");
///
/// let result = nats.next_message();
/// assert!(result.is_ok());
/// let message = result.expect("Received Message");
/// println!("{} -> {}", message.subject, message.data);
/// ```
// =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
impl SubscribeClient {
    pub fn new(host: &str, root: &str, subject: &str) -> Result<Self, Error> {
        let mut socket = Socket::new(host, "NATS Subscriber", 30);

        // Get Client ID
        let info_line = match socket.readln() {
            Ok(line) => line,
            Err(_) => return Err(Error::Initialize),
        };
        let data = match info_line.trim().split_whitespace().nth(1) {
            Some(data) => data,
            None => return Err(Error::Initialize),
        };
        let info = match json::parse(data) {
            Ok(info) => info,
            Err(_) => return Err(Error::Initialize),
        };
        let client_id = info["client_id"].to_string();

        let mut nats = Self {
            socket,
            client_id,
            root: root.into(),
            subject: subject.into(),
        };

        nats.subscribe();
        Ok(nats)
    }

    /// ## Receive NATS Messages
    ///
    /// Subscribe to any NATS subject.
    /// > Warning: This method can only be called once per client because
    /// > NATS does not support multiplexing.
    /// > If you need multiple subjects, initialize one NATS client per
    /// > subject and put each client into a thread.
    ///
    /// ```no_run
    /// use nats_bridge::nats::SubscribeClient;
    ///
    /// let root = "subjects"; // subjects.demo
    /// let subject = "demo";  // subjects.demo
    /// let mut nats = SubscribeClient::new("0.0.0.0:4222", root, subject)
    ///     .expect("NATS Subscribe Client");
    ///
    /// let message = nats.next_message().expect("Received Message");
    /// ```
    fn subscribe(&mut self) {
        loop {
            let subject = if self.root.is_empty() {
                self.subject.to_string()
            } else {
                format!(
                    "{root}.{subject}",
                    subject = self.subject,
                    root = self.root
                )
            };

            let sub = format!(
                "SUB {subject} {client_id}\r\n",
                subject = subject,
                client_id = self.client_id,
            );
            match self.socket.write(sub) {
                Ok(_) => break,
                Err(_) => self.subscribe(),
            };
        }
    }

    /// ## Receive NATS Messages
    ///
    /// Easy way to get messages from the initialized subject.
    ///
    /// ```no_run
    /// use nats_bridge::nats::SubscribeClient;
    ///
    /// let root = "subjects"; // subjects.demo
    /// let subject = "demo";  // subjects.demo
    /// let mut nats = SubscribeClient::new("0.0.0.0:4222", root, subject)
    ///     .expect("NATS Subscribe Client");
    ///
    /// let message = nats.next_message().expect("Received Message");
    /// ```
    pub fn next_message(&mut self) -> Result<Message, Error> {
        loop {
            let data = match self.socket.readln() {
                Ok(data) => data,
                Err(_) => {
                    self.subscribe();
                    return Err(Error::Subscribe);
                }
            };

            let detail: Vec<_> = data.trim().split_whitespace().collect();
            if detail.is_empty() {
                continue;
            }

            let command = detail[0];
            match command {
                "PING" => {
                    match self.socket.write("PONG\r\n") {
                        Ok(_) => {}
                        Err(_) => self.subscribe(),
                    };
                }
                "MSG" => {
                    if detail.len() != 4 {
                        continue;
                    }

                    let message = match self.socket.readln() {
                        Ok(message) => message,
                        Err(_) => {
                            self.subscribe();
                            return Err(Error::Subscribe);
                        }
                    };

                    let source: String = detail[1].into();

                    let subject = if self.root.is_empty() {
                        source
                    } else {
                        source[self.root.len() + 1..].to_string()
                    };

                    return Ok(Message {
                        root: self.root.to_string(),
                        subject,
                        my_id: detail[2].into(),
                        sender_id: detail[3].into(),
                        data: message.trim().into(),
                    });
                }
                _ => continue,
            }
        }
    }

    #[cfg(test)]
    pub fn ping(&mut self) -> Result<String, Error> {
        let _size = match self.socket.write("PING\r\n") {
            Ok(size) => size,
            Err(_error) => return Err(Error::Ping),
        };
        match self.socket.readln() {
            Ok(data) => Ok(data),
            Err(_error) => Err(Error::Ping),
        }
    }

    #[cfg(test)]
    pub fn exit(&mut self) -> Result<(), Error> {
        match self.socket.write("EXIT\r\n") {
            Ok(_size) => Ok(()),
            Err(_error) => Err(Error::Exit),
        }
    }
}

impl Drop for SubscribeClient {
    fn drop(&mut self) {
        self.socket.disconnect();
    }
}

// =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
/// # NATS Publish Client
///
/// This client lib offers publish support to NATS.
///
/// ```no_run
/// use nats_bridge::nats::PublishClient;
///
/// let mut nats = PublishClient::new("0.0.0.0:4222", "").expect("NATS PUB");
///
/// loop {
///     let result = nats.publish("hello", "subject");
///     assert!(result.is_ok());
/// }
/// ```
///
// =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
impl PublishClient {
    pub fn new(host: &str, root: &str) -> Result<Self, Error> {
        let mut socket = Socket::new(host, "NATS Publisher", 5);
        let _infoln = match socket.readln() {
            Ok(data) => data,
            Err(_) => return Err(Error::Initialize),
        };

        Ok(Self {
            socket,
            root: root.into(),
        })
    }

    /// ## Send NATS Messages
    ///
    /// Easy way to send messages to any NATS subject.
    ///
    /// ```no_run
    /// use nats_bridge::nats::PublishClient;
    ///
    /// let subject = "demo";
    /// let root = "root";
    /// let mut nats = PublishClient::new("0.0.0.0:4222", root)
    ///     .expect("NATS Publish Client");
    ///
    /// nats.publish(subject, "Hello").expect("publish sent");
    /// ```
    pub fn publish(
        &mut self,
        subject: impl AsRef<str>,
        data: impl AsRef<str>,
    ) -> Result<(), Error> {
        let subject = if self.root.is_empty() {
            subject.as_ref().to_string()
        } else {
            format!(
                "{root}.{subject}",
                subject = subject.as_ref(),
                root = self.root
            )
        };

        let pubcmd = format!(
            "PUB {subject} {length}\r\n{data}\r\n",
            subject = subject,
            length = data.as_ref().len(),
            data = data.as_ref(),
        );
        match self.socket.write(pubcmd) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::Publish),
        }
    }

    #[cfg(test)]
    pub fn ping(&mut self) -> Result<String, Error> {
        let _size = match self.socket.write("PING\r\n") {
            Ok(size) => size,
            Err(_error) => return Err(Error::Ping),
        };
        match self.socket.readln() {
            Ok(data) => Ok(data),
            Err(_error) => Err(Error::Ping),
        }
    }

    #[cfg(test)]
    pub fn exit(&mut self) -> Result<(), Error> {
        match self.socket.write("EXIT\r\n") {
            Ok(_size) => Ok(()),
            Err(_error) => Err(Error::Exit),
        }
    }
}

impl Drop for PublishClient {
    fn drop(&mut self) {
        self.socket.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    struct NATSMock {
        listener: TcpListener,
    }

    impl NATSMock {
        fn new(host: &str) -> std::io::Result<Self> {
            let listener = TcpListener::bind(host)?;
            Ok(Self { listener })
        }

        fn process(&self) {
            match self.listener.accept() {
                Ok((mut socket, _addr)) => {
                    socket.write_all(b"INFO {\"server_id\":\"asbLGfs3r7pgZwucUxYnPn\",\"version\":\"1.4.1\",\"proto\":1,\"git_commit\":\"3e64f0b\",\"go\":\"go1.11.5\",\"host\":\"0.0.0.0\",\"port\":4222,\"max_payload\":1048576,\"client_id\":9999}\r\n").expect("Could not send info");

                    let mut reader = BufReader::new(
                        socket.try_clone().expect("Unable to clone socket"),
                    );
                    let mut line = String::new();

                    loop {
                        line.clear();
                        let size = reader
                            .read_line(&mut line)
                            .expect("Unable to read line");
                        if size == 0 {
                            eprintln!("Socket disconnected while reading");
                            break;
                        }

                        match line.as_ref() {
                            "EXIT\r\n" => break,
                            "PING\r\n" => {
                                socket
                                    .write_all(b"PONG\r\n")
                                    .expect("Unable to write");
                            }
                            "SUB demo 9999\r\n" => {
                                socket
                                    .write_all(
                                        b"MSG demo 9999 5\r\nKNOCK\r\n",
                                    )
                                    .expect("Unable to write");
                            }
                            "PUB demo 5\r\n" => {
                                line.clear();
                                reader
                                    .read_line(&mut line)
                                    .expect("Unable to read line");

                                let cmd = format!("MSG demo 1 1\r\n{}", line);
                                socket
                                    .write_all(cmd.as_bytes())
                                    .expect("Unable to write");
                            }
                            _ => eprintln!("Unexpected line: `{}`", line),
                        };
                    }
                }
                Err(e) => eprintln!("couldn't get client: {:?}", e),
            }
        }
    }

    #[test]
    fn publish_ok() {
        let host = "0.0.0.0:4220";
        let mock = NATSMock::new(host).expect("Unable to listen");
        let t = thread::spawn(move || {
            mock.process();
        });

        let subject = "demo";
        let root = "";
        let mut publisher =
            PublishClient::new(host, root).expect("NATS Publish Client");

        publisher.publish(subject, "Hello").expect("Message Sent");
        publisher.exit().expect("NATS Connection Closed");
        t.join().expect("Mock TcpStream server");
    }

    #[test]
    fn subscribe_ok() {
        let host = "0.0.0.0:4221";
        let mock = NATSMock::new(host).expect("Unable to listen");
        let t = thread::spawn(move || {
            mock.process();
        });

        let subject = "demo";
        let root = "";
        let mut subscriber = SubscribeClient::new(host, root, subject)
            .expect("NATS Subscribe Client");
        let result = subscriber.next_message();
        assert!(result.is_ok());
        let message = result.expect("Received Message");
        assert!(message.subject.len() > 0);
        subscriber.exit().expect("NATS Socket Closed");
        t.join().expect("Mock TcpStream server");
    }

    #[test]
    fn ping_ok() {
        let host = "0.0.0.0:4223";
        let root = "";
        let mock = NATSMock::new(host).expect("Unable to listen");
        let t = thread::spawn(move || {
            mock.process();
        });

        let mut nats =
            PublishClient::new(host, root).expect("NATS Publish Client");

        let pong = nats.ping().expect("Pong from Ping");
        assert_eq!(pong, "PONG\r\n");

        nats.exit().expect("NATS Connection Closed");
        t.join().expect("Thread died early...");
    }
}
