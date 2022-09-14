use std::sync::{Arc, Mutex};
use std::os::unix::io::{RawFd, AsRawFd};
use tokio_pipe::{PipeRead, PipeWrite};
use circbuf::CircBuf;
use anyhow::{Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io::Write;

const APP_LOG_CAPACITY: usize = 128*1024;

pub struct LogCursor {
    pos: usize,
}

impl LogCursor {
    fn new() -> Self {
        return Self{
            pos: 0usize,
        }
    }
}

struct ByteLog {
    buffer: CircBuf,
    head: usize,
}

impl ByteLog {
    fn new() -> Self {
        Self{
            buffer: CircBuf::with_capacity(APP_LOG_CAPACITY).unwrap(),
            head: 0usize,
        }
    }

    // returns the number of bytes it trimmed from the head
    fn append(&mut self, data: &[u8]) -> usize {
        let mut trim_cnt = 0usize;

        let avail = self.buffer.avail();
        if avail < data.len() {
            trim_cnt = data.len() - avail;
            self.buffer.advance_read(trim_cnt);
            self.head += trim_cnt;
        }
        assert!(self.buffer.avail() >= data.len());

        assert!(self.buffer.write(data).unwrap() == data.len());

        trim_cnt
    }

    fn read(&self, cursor: &mut LogCursor, mut buf: &mut [u8]) -> usize {
        let mut copied = 0usize;

        let mut offset = if cursor.pos < self.head {
            cursor.pos = self.head;
            0usize
        } else {
            cursor.pos - self.head
        };

        for mut data in self.buffer.get_bytes_upto_size(buf.len() + offset) {
            if offset < data.len() {
                data = &data[offset..];
                offset = 0;

                buf[..data.len()].copy_from_slice(data);
                buf = &mut buf[data.len()..];

                copied += data.len();
            } else {
                offset -= data.len();
            }
        }

        cursor.pos += copied;

        copied
    }

    fn cap(&self) -> usize {
        self.buffer.cap()
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }
}

pub struct LogWriter {
    w_pipe: PipeWrite,
}

pub struct LogServicer {
    r_pipe: PipeRead,
    log: Arc<Mutex<ByteLog>>,
}

pub struct LogReader {
    log: Arc<Mutex<ByteLog>>,
}

pub fn new_app_log() -> Result<(LogWriter, LogServicer, LogReader)> {
    let (r, w) = tokio_pipe::pipe()?;

    let log = Arc::new(Mutex::new(ByteLog::new()));

    let lw = LogWriter{
        w_pipe: w,
    };

    let ls = LogServicer {
        r_pipe: r,
        log: log.clone(),
    };

    let lr = LogReader{
        log: log,
    };

    Ok((lw, ls, lr))
}

impl LogWriter {
    pub fn redirect_stdio(&self) -> Result<()> {
        nix::unistd::dup2(self.w_pipe.as_raw_fd(), std::io::stdout().as_raw_fd())?;
        nix::unistd::dup2(self.w_pipe.as_raw_fd(), std::io::stderr().as_raw_fd())?;

        Ok(())
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.w_pipe.as_raw_fd()
    }

    pub async fn write(&mut self, data: &[u8]) -> Result<()> {
        self.w_pipe.write_all(data).await?;
        Ok(())
    }
}

impl LogServicer {
    // run in the background and pull data off of the pipe
    pub async fn run(&mut self) -> Result<()> {
        let mut buf = vec![0u8; 16*1024];
        loop {
            let n = self.r_pipe.read(&mut buf).await?;
            if n == 0 {
                return Ok(());
            }

            self.log.lock().unwrap().append(&buf[..n]);
        }
    }
}

impl LogReader {
    pub fn read(&self, cursor: &mut LogCursor, buf: &mut [u8]) -> usize {
       self.log.lock().unwrap().read(cursor, buf)
    }

    pub fn len(&self) -> usize {
        self.log.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use assert2::assert;
    use super::{ByteLog, LogCursor};

    fn check_log(log: &ByteLog, mut expected: u8) {
        // check that the log contents monotonically increase
        let mut c = LogCursor::new();

        let mut buf = vec![0u8; 1024];

        loop {
            match log.read(&mut c, &mut buf) {
                0 => break,
                nread =>  {
                    for actual in &buf[..nread] {
                        assert!(*actual == expected);
                        expected = expected.wrapping_add(1u8);
                    }
                }
            }
        }
    }

    fn iota_u8(slice: &mut [u8], mut start: u8) -> u8 {
        for x in slice {
            *x = start;
            start = start.wrapping_add(1u8);
        }

        start
    }

    #[test]
    fn test_byte_log() {
        let mut log = ByteLog::new();

        // append by a bit upto the log capacity
        let mut logged = 0usize;
        let mut quanta = 5;
        let mut i = 0u8;
        while logged < log.cap() - quanta {
            let mut data = vec![0u8; quanta];
            i = iota_u8(&mut data, i);

            assert!(log.append(&data) == 0usize);
            quanta += 1;
            logged += data.len();

            check_log(&log, 0);
        }

        let mut expected = 0u8;

        // overflow the log so it starts to trim
        while logged < log.cap() * 3 {
            let mut data = vec![0u8; quanta];
            i = iota_u8(&mut data, i);

            let trimmed = log.append(&data);
            assert!(trimmed > 0);
            quanta += 1;

            expected = expected.wrapping_add(trimmed as u8);
            check_log(&log, expected);

            logged += data.len();
        }
    }

    #[tokio::test]
    async fn test_app_log() {
        use rand::RngCore;
        use std::time::Duration;

        let (mut w, mut s, r) = super::new_app_log().unwrap();

        let runner = tokio::spawn(async move {
            s.run().await.unwrap();
        });

        let mut expected = vec![0u8; super::APP_LOG_CAPACITY*3];
        rand::thread_rng().fill_bytes(&mut expected);

        // write all in small chunks
        for chunk in expected.chunks(53) {
            w.write(chunk).await.unwrap();
        }

        tokio::time::sleep(Duration::from_secs(1)).await;

        runner.abort();
        assert!(runner.await.unwrap_err().is_cancelled());

        // read the tail
        let mut c = LogCursor::new();
        let mut actual: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; 1024];
        loop {
            let nread = r.read(&mut c, &mut buf);
            if nread == 0 {
                break;
            }

            actual.extend_from_slice(&mut buf[..nread]);
        }

        assert!(actual.len() == r.len());

        let tail_pos = expected.len() - actual.len();
        assert!(actual == expected[tail_pos..]);
    }
}
