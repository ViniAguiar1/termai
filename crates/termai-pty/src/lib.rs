use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};

/// Managed PTY session wrapping a shell process.
pub struct PtySession {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl PtySession {
    /// Spawn a new shell in a PTY with the given dimensions.
    pub fn spawn(cols: u16, rows: u16) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(std::env::current_dir().unwrap_or_else(|_| "/".into()));

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            reader,
            writer,
            _child: child,
        })
    }

    /// Read available bytes from PTY output.
    pub fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }

    /// Write input bytes to the PTY (keyboard input).
    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()
    }
}
