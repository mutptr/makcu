use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::{mpsc, watch};

use crate::Makcu;

pub struct InputEmulator {
    is_locked: AtomicBool,
    is_pending: AtomicBool,
    pending_tx: mpsc::Sender<()>,
    current_press: AtomicBool,
    user_press: AtomicBool,
    makcu: Makcu,
}

impl InputEmulator {
    pub fn new(makcu: Makcu) -> Arc<Self> {
        let (pending_tx, pending_rx) = mpsc::channel::<()>(100);
        let is_locked = AtomicBool::new(false);
        let is_pending = AtomicBool::new(false);
        let current_press = AtomicBool::new(false);
        let user_press = AtomicBool::new(false);

        let emulator = Arc::new(Self {
            is_locked,
            is_pending,
            pending_tx,
            current_press,
            user_press,
            makcu,
        });

        tokio::spawn(userpress_task(
            emulator.makcu.subscribe_buttons(),
            Arc::clone(&emulator),
        ));

        // pending task 시작
        tokio::spawn(pending_task(pending_rx, Arc::clone(&emulator)));

        emulator
    }

    pub async fn lock(&self) -> anyhow::Result<()> {
        let was_locked = self.is_locked.swap(true, Ordering::AcqRel);
        if !was_locked {
            self.makcu.lock_ml().await?;
            self.makcu.lock_ms1().await?;
        }
        Ok(())
    }

    pub async fn unlock(&self) -> anyhow::Result<()> {
        let was_locked = self.is_locked.swap(false, Ordering::AcqRel);
        if was_locked {
            self.makcu.unlock_ml().await?;
            self.makcu.unlock_ms1().await?;
            self.release().await?;
        }
        Ok(())
    }

    pub async fn pending(&self, pending: bool) -> anyhow::Result<()> {
        let was_pending = self.is_pending.swap(pending, Ordering::AcqRel);
        if was_pending {
            _ = self.pending_tx.send(()).await;
        } else {
            self.sync_current_state().await;
        }
        Ok(())
    }

    pub async fn press(&self) -> anyhow::Result<()> {
        let was_pressed = self.current_press.swap(true, Ordering::AcqRel);
        if !was_pressed {
            self.makcu.press().await?;
        }
        Ok(())
    }

    pub async fn release(&self) -> anyhow::Result<()> {
        let was_pressed = self.current_press.swap(false, Ordering::AcqRel);
        if was_pressed {
            self.makcu.release().await?;
        }
        Ok(())
    }

    pub async fn mouse_move(&self, x: i32, y: i32) -> anyhow::Result<()> {
        self.makcu.mouse_move(x, y).await?;
        Ok(())
    }

    pub async fn click(&self) -> anyhow::Result<()> {
        let current_pressed = self.current_press.load(Ordering::Acquire);
        if !current_pressed {
            self.makcu.click().await?;
        }
        Ok(())
    }

    pub async fn sync_current_state(&self) {
        let user_pressed = self.user_press.load(Ordering::Acquire);
        if user_pressed {
            let _ = self.press().await;
        } else {
            let _ = self.release().await;
        }
    }
}

async fn userpress_task(mut key_state: watch::Receiver<u8>, emulator: Arc<InputEmulator>) {
    while key_state.changed().await.is_ok() {
        let key = *key_state.borrow();
        let user_press = (key & 1) == 1;
        emulator.user_press.store(user_press, Ordering::Release);

        if !emulator.is_locked.load(Ordering::Acquire) {
            continue;
        }

        let ms1_press = key >> 3 & 1 == 1;
        let user_press = user_press || ms1_press;
        emulator.user_press.store(user_press, Ordering::Release);

        if ms1_press {
            emulator.sync_current_state().await;
            continue;
        }

        if user_press && emulator.is_pending.load(Ordering::Acquire) {
            continue;
        }

        emulator.sync_current_state().await;
    }
}

async fn pending_task(mut pending_rx: mpsc::Receiver<()>, emulator: Arc<InputEmulator>) {
    while pending_rx.recv().await.is_some() {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    tracing::debug!("Auto unpending");
                    _ = emulator.pending(false).await;
                    break;
                }
                new_msg = pending_rx.recv() => {
                    match new_msg {
                        Some(_) => continue,
                        None => return,
                    }
                }
            }
        }
    }
}
