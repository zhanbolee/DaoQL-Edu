// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::DaoQLError;
use crate::wal::record::{WalRecord, WAL_MAGIC};

/// WAL writer
pub struct WalWriter {
    /// target file
    file: Arc<Mutex<File>>,
    /// foreground buffer (Buffer A)
    buffer_a: Vec<u8>,
    /// background buffer (Buffer B)
    buffer_b: Vec<u8>,
    /// monotonically increasing sequence number
    seq: AtomicU64,
    /// switch threshold (bytes)
    switch_threshold: usize,
    /// Flush interval (currently triggered by append reaching threshold, this field reserved for future background thread)
    #[allow(dead_code)]
    flush_interval: Duration,
    /// background thread handle
    bg_thread: Option<thread::JoinHandle<()>>,
    /// background thread control
    shutdown_flag: Arc<AtomicBool>,
    /// condition variable: notify background thread has data
    cond: Arc<(Mutex<bool>, Condvar)>,
    /// last flush time
    last_flush: Instant,
    /// Whether to execute fsync on flush (can disable in benchmarks to exclude I/O noise)
    sync_on_flush: bool,
}

impl WalWriter {
    /// Create WAL writer
    pub fn new(
        path: impl AsRef<Path>,
        buffer_size: usize,
        flush_interval_ms: u64,
        sync_on_flush: bool,
    ) -> Result<Self, DaoQLError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        // Writemagic number（file header）
        let mut file = file;
        let metadata = file.metadata()?;
        if metadata.len() == 0 {
            file.write_all(&WAL_MAGIC)?;
            file.write_all(&[0u8; 4])?; // version/reserved
            file.sync_all()?;
        }

        let file = Arc::new(Mutex::new(file));
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let cond = Arc::new((Mutex::new(false), Condvar::new()));

        let _bg_file = file.clone();
        let bg_shutdown = shutdown_flag.clone();
        let bg_cond = cond.clone();

        // start background flush thread
        let bg_thread = thread::spawn(move || {
            while !bg_shutdown.load(Ordering::Relaxed) {
                let (lock, cvar) = &*bg_cond;
                let mut ready = lock.lock().unwrap();
                let result = cvar.wait_timeout(ready, Duration::from_millis(100));
                ready = result.unwrap().0;
                *ready = false;
                drop(ready);

                // background thread periodically checks and flushes
                // actual flush triggered by foreground swap_buffers
            }
        });

        Ok(Self {
            file,
            buffer_a: Vec::with_capacity(buffer_size),
            buffer_b: Vec::with_capacity(buffer_size),
            seq: AtomicU64::new(1),
            switch_threshold: buffer_size,
            flush_interval: Duration::from_millis(flush_interval_ms),
            bg_thread: Some(bg_thread),
            shutdown_flag,
            cond,
            last_flush: Instant::now(),
            sync_on_flush,
        })
    }

    /// append WAL record
    ///
    /// Thread safety: &mut self guarantees single-threaded access
    pub fn append(&mut self, record: &WalRecord) -> Result<u64, DaoQLError> {
        let bytes = record.serialize();
        let seq = record.seq;

        // check whether buffer swap is needed (only when buffer is full)
        // Note: don't check time during append, avoid system calls on hot path
        // background thread responsible for timed flush
        if self.buffer_a.len() + bytes.len() > self.switch_threshold {
            self.swap_buffers()?;
        }

        self.buffer_a.extend_from_slice(&bytes);
        Ok(seq)
    }

    /// Swap buffer and flush
    fn swap_buffers(&mut self) -> Result<(), DaoQLError> {
        if self.buffer_a.is_empty() {
            return Ok(());
        }

        // Swap A ↔ B
        std::mem::swap(&mut self.buffer_a, &mut self.buffer_b);
        self.buffer_a.clear();
        self.buffer_a.reserve(self.switch_threshold);

        // Flush Buffer B
        let buf = std::mem::take(&mut self.buffer_b);
        let mut file = self.file.lock().unwrap();
        file.write_all(&buf)?;
        if self.sync_on_flush {
            file.sync_all()?; // fsync
        }
        drop(file);

        self.last_flush = Instant::now();

        // notify background thread
        let (lock, cvar) = &*self.cond;
        let mut ready = lock.lock().unwrap();
        *ready = true;
        cvar.notify_one();

        Ok(())
    }

    /// forceFlush（synchronous）
    pub fn flush(&mut self) -> Result<(), DaoQLError> {
        self.swap_buffers()?;
        Ok(())
    }

    /// Closewriter
    pub fn shutdown(&mut self) -> Result<(), DaoQLError> {
        self.flush()?;
        self.shutdown_flag.store(true, Ordering::Relaxed);
        let (lock, cvar) = &*self.cond;
        let mut ready = lock.lock().unwrap();
        *ready = true;
        cvar.notify_all();
        drop(ready);

        if let Some(handle) = self.bg_thread.take() {
            let _ = handle.join();
        }
        Ok(())
    }

    /// current sequence number
    pub fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }
}

impl Drop for WalWriter {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::wal::record::TransactionPayload;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("daoql-edu-test-wal");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_wal_writer_append_and_flush() {
        let path = temp_path("test_append.wal");
        let _ = std::fs::remove_file(&path);

        let mut writer = WalWriter::new(&path, 1024, 1000, true).unwrap();

        for i in 0..10 {
            let payload = TransactionPayload::new(i);
            let record = WalRecord::new(writer.next_seq(), payload.to_bytes().unwrap());
            writer.append(&record).unwrap();
        }

        writer.flush().unwrap();

        // validatefilesize
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 8); // at least contains magic number
    }

    #[test]
    fn test_wal_writer_buffer_switch() {
        let path = temp_path("test_switch.wal");
        let _ = std::fs::remove_file(&path);

        // small buffer, triggers swap
        let mut writer = WalWriter::new(&path, 64, 1000, true).unwrap();

        for _ in 0..100 {
            let payload = TransactionPayload::new(1);
            let bytes = payload.to_bytes().unwrap();
            let record = WalRecord::new(writer.next_seq(), bytes);
            writer.append(&record).unwrap();
        }

        writer.shutdown().unwrap();

        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 100); // should have lots of data
    }
}
