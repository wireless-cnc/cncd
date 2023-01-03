use tokio::sync::mpsc::{channel, Sender as TokioSender};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    task,
};
use tokio_serial::SerialPortBuilderExt;
pub struct SerialComm {
    gcode_sender: TokioSender<String>,
}

impl SerialComm {
    pub fn new(tty: &str, baud_rate: u32, response_sender: TokioSender<String>) -> Self {
        let port = tokio_serial::new(tty, baud_rate)
            .open_native_async()
            .expect("Failed to open the system serial port");
        let (mut reader, mut writer) = io::split(port);
        let (gcode_sender, mut gcode_receiver) = channel::<String>(100);
        let tty_clone1 = String::from(tty);
        let tty_clone2 = tty_clone1.clone();
        let reader_handle = task::spawn(async move {
            log::info!("Started a reader on {}", tty_clone1);
            let mut buf = [0u8; 1024];
            let mut string_buf = String::from("");
            loop {
                match reader.read(&mut buf).await {
                    Ok(n) => {
                        string_buf +=
                            &String::from(String::from_utf8_lossy(&buf[..n]).into_owned());
                        let res: Vec<&str> = string_buf.split("\n").collect();
                        for line in &res[..res.len() - 1] {
                            log::debug!("< {}", line.to_string());
                            response_sender
                                .send(line.to_string())
                                .await
                                .expect("Failed to send response from serial to channel");
                        }
                        if res[res.len() - 1] == "" {
                            string_buf.clear();
                        } else {
                            string_buf = res[res.len() - 1].to_string();
                        }
                    }
                    Err(e) => {
                        log::warn!("Reader has encountered an error: {}", e);
                        break;
                    }
                }
            }
            log::info!("Reader has exited");
        });
        task::spawn(async move {
            log::info!("Started a writer on {}", tty_clone2);
            loop {
                let msg = gcode_receiver.recv().await;
                match msg {
                    Some(msg) => {
                        log::debug!("> {}", msg.trim_end());
                        writer
                            .write_all(msg.as_bytes())
                            .await
                            .expect("failed to write to serial port");
                    }
                    None => {
                        break;
                    }
                }
            }
            log::info!("Writer has exited. Shutting down reader...");
            reader_handle.abort();
            match reader_handle.await {
                Ok(_) => {
                    log::info!("Reader has exited cleanly");
                }
                Err(_) => {
                    log::warn!("Reader has exited (aborted)");
                }
            }
        });
        return SerialComm { gcode_sender };
    }
    pub fn get_sender(&self) -> TokioSender<String> {
        return self.gcode_sender.clone();
    }
}
