use async_std;
use async_std::net::TcpStream;
use async_tungstenite::{async_std::connect_async, tungstenite::Message, WebSocketStream};
use failure::{format_err, Error};
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Debug, PartialEq)]
pub enum FileType {
    File,
    Dir,
}

#[derive(Debug)]
pub struct FileInfo {
    pub ty: FileType,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug)]
enum Opcode {
    Attach,
    DeviceList,
    GetAddress,
    Info,
    List,
    PutFile,
    Remove,
}

#[derive(Deserialize, Serialize, Debug)]
enum Space {
    #[serde(rename = "SNES")]
    Snes,
}

#[derive(Deserialize, Serialize, Debug)]
struct Request {
    #[serde(rename = "Opcode")]
    opcode: Opcode,

    #[serde(rename = "Space")]
    space: Space,

    #[serde(rename = "Flags")]
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<Vec<String>>,

    #[serde(rename = "Operands")]
    #[serde(skip_serializing_if = "Option::is_none")]
    ops: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Results {
    #[serde(rename = "Results")]
    results: Vec<String>,
}

pub struct Connection {
    ws: WebSocketStream<TcpStream>,
    attached: bool,
}

impl Connection {
    pub async fn new(addr: &str) -> Result<Connection, Error> {
        let (ws_stream, _) = connect_async(addr).await?;

        Ok(Connection {
            ws: ws_stream,
            attached: false,
        })
    }

    pub async fn close(mut self) -> Result<(), Error> {
        self.ws.close(None).await?;
        Ok(())
    }

    async fn send(&mut self, data: &Request) -> Result<(), Error> {
        let json = serde_json::to_string(data)?;
        self.ws.send(json.into()).await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Vec<String>, Error> {
        use futures::prelude::*;
        while let Some(msg) = self.ws.next().await {
            let msg = msg?;
            if msg.is_text() || msg.is_binary() {
                let results: Results = serde_json::from_str(&msg.to_string())?;
                return Ok(results.results);
            }
        }
        Err(format_err!("no message"))
    }

    pub async fn get_device_list(&mut self) -> Result<Vec<String>, Error> {
        let req = Request {
            opcode: Opcode::DeviceList,
            space: Space::Snes,
            flags: None,
            ops: None,
        };
        self.send(&req).await?;
        self.recv().await
    }

    pub async fn attach(&mut self, device: &str) -> Result<(), Error> {
        let req = Request {
            opcode: Opcode::Attach,
            space: Space::Snes,
            flags: None,
            ops: Some(vec![device.to_string()]),
        };
        self.send(&req).await?;
        self.attached = true;
        Ok(())
    }

    pub async fn get_info(&mut self) -> Result<Vec<String>, Error> {
        if !self.attached {
            return Err(format_err!("Not attached to device"));
        }

        let req = Request {
            opcode: Opcode::Info,
            space: Space::Snes,
            flags: None,
            ops: None,
        };
        self.send(&req).await?;
        self.recv().await
    }

    pub async fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, Error> {
        // server will lock up if we query a path ending with /
        let path = path.trim_end_matches('/');

        let req = Request {
            opcode: Opcode::List,
            space: Space::Snes,
            flags: None,
            ops: Some(vec![path.to_string()]),
        };
        self.send(&req).await?;
        let mut files = Vec::new();
        for strs in self.recv().await?.chunks(2) {
            files.push(FileInfo {
                ty: if strs[0] == "0" {
                    FileType::Dir
                } else {
                    FileType::File
                },
                name: strs[1].clone(),
            });
        }

        Ok(files)
    }

    pub async fn put_file(&mut self, path: &str, data: &[u8]) -> Result<(), Error> {
        let req = Request {
            opcode: Opcode::PutFile,
            space: Space::Snes,
            flags: None,
            ops: Some(vec![path.to_string(), format!("{:X}", data.len())]),
        };

        self.send(&req).await?;

        for chunk in data.chunks(1024) {
            self.ws.send(Message::Binary(chunk.to_vec())).await?;
            self.ws.flush().await?;
        }

        Ok(())
    }

    pub async fn rm(&mut self, path: &str) -> Result<(), Error> {
        let req = Request {
            opcode: Opcode::Remove,
            space: Space::Snes,
            flags: None,
            ops: Some(vec![path.to_string()]),
        };
        self.send(&req).await?;
        self.ws.flush().await?;
        Ok(())
    }

    pub async fn read_mem(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Error> {
        let mut offset = 0;
        let len = data.len();
        let req = Request {
            opcode: Opcode::GetAddress,
            space: Space::Snes,
            flags: None,
            ops: Some(vec![format!("{:X}", addr), format!("{:X}", len)]),
        };
        self.send(&req).await?;

        while offset < len {
            while let Some(msg) = self.ws.next().await {
                let msg = msg?;
                if msg.is_binary() {
                    let msg_data = msg.into_data();
                    let read_size = msg_data.len();
                    data[offset..offset + read_size].clone_from_slice(&msg_data);
                    offset += read_size;
                    break;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn request_encoding() -> Result<(), Error> {
        struct ReqTest {
            req: Request,
            json: String,
        };

        let tests = vec![
            ReqTest {
                req: Request {
                    opcode: Opcode::DeviceList,
                    space: Space::Snes,
                    flags: None,
                    ops: None,
                },
                json: r#"{
  "Opcode": "DeviceList",
  "Space": "SNES"
}"#
                .to_string(),
            },
            ReqTest {
                req: Request {
                    opcode: Opcode::Attach,
                    space: Space::Snes,
                    flags: None,
                    ops: Some(vec!["SD2SNES COM3".to_string()]),
                },
                json: r#"{
  "Opcode": "Attach",
  "Space": "SNES",
  "Operands": [
    "SD2SNES COM3"
  ]
}"#
                .to_string(),
            },
        ];

        for test in tests {
            let enc = serde_json::to_string_pretty(&test.req)?;
            assert_eq!(enc, test.json);
        }

        Ok(())
    }
}
