use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum ProcessEvent {
    Output {
        service_index: usize,
        data: Vec<u8>,
    },
    Exited {
        service_index: usize,
        success: bool,
        code: Option<u32>,
    },
}

pub struct PtyProcess {
    pub service_index: usize,
    child: Box<dyn Child + Send + Sync>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    is_alive: Arc<AtomicBool>,
}

impl PtyProcess {
    pub fn spawn(
        service_index: usize,
        command: &str,
        args: &[String],
        directory: &Path,
        cols: u16,
        rows: u16,
        tx: UnboundedSender<ProcessEvent>,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.max(10),
                cols: cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY pair")?;

        let mut cmd = CommandBuilder::new(command);
        cmd.args(args);
        cmd.cwd(directory);

        // Inherit PATH and ensure TERM is xterm-256color
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("FORCE_COLOR", "1");

        let child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("Failed to spawn command '{}'", command))?;

        // Drop the slave immediately in the parent so the master sees EOF when the child exits
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY master reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("Failed to get PTY master writer")?;

        let is_alive = Arc::new(AtomicBool::new(true));
        let is_alive_clone = Arc::clone(&is_alive);

        // Spawn a background thread to read output continuously until EOF
        thread::Builder::new()
            .name(format!("pty-reader-{}", service_index))
            .spawn(move || {
                let mut buffer = [0u8; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break, // EOF reached
                        Ok(n) => {
                            let data = buffer[..n].to_vec();
                            if tx.send(ProcessEvent::Output {
                                service_index,
                                data,
                            }).is_err() {
                                break;
                            }
                        }
                        Err(_) => break, // Process closed or error
                    }
                }
                is_alive_clone.store(false, Ordering::SeqCst);
                let _ = tx.send(ProcessEvent::Exited {
                    service_index,
                    success: true,
                    code: Some(0),
                });
            })
            .context("Failed to spawn PTY reader thread")?;

        Ok(Self {
            service_index,
            child,
            master: pair.master,
            writer,
            is_alive,
        })
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data).context("Failed to write to PTY stdin")?;
        self.writer.flush().context("Failed to flush PTY stdin")?;
        Ok(())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows: rows.max(5),
                cols: cols.max(10),
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to resize PTY")?;
        Ok(())
    }

    pub fn kill(&mut self) -> Result<()> {
        if self.is_alive.load(Ordering::SeqCst) {
            let _ = self.child.kill();
            self.is_alive.store(false, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn is_alive(&self) -> bool {
        self.is_alive.load(Ordering::SeqCst)
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc::unbounded_channel;

    #[tokio::test]
    async fn test_pty_spawn_and_read_output() {
        let (tx, mut rx) = unbounded_channel();
        let temp_dir = std::env::temp_dir();

        let mut proc = PtyProcess::spawn(
            0,
            "sh",
            &["-c".to_string(), "echo tx_pty_test_ok".to_string()],
            &temp_dir,
            80,
            24,
            tx,
        )
        .unwrap();

        assert_eq!(proc.service_index, 0);

        let mut collected_output = Vec::new();
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Ok(event) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                match event {
                    Some(ProcessEvent::Output { data, .. }) => {
                        collected_output.extend_from_slice(&data);
                    }
                    Some(ProcessEvent::Exited { .. }) => break,
                    None => break,
                }
            }
        }

        let output_str = String::from_utf8_lossy(&collected_output);
        assert!(
            output_str.contains("tx_pty_test_ok"),
            "Expected 'tx_pty_test_ok' in output, got: {}",
            output_str
        );

        let _ = proc.kill();
    }
}
