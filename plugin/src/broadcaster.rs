use {
    crate::{config::BroadcasterConfig, message::SlotMessage},
    tokio::{
        net::UdpSocket,
        select,
        sync::mpsc::{self, Sender},
        task::JoinHandle,
    },
    tokio_util::sync::CancellationToken,
};

#[derive(Debug)]
pub(crate) struct Broadcaster {
    handle: JoinHandle<Result<(), anyhow::Error>>,
    cancel: CancellationToken,
}

impl Broadcaster {
    pub async fn run(
        config: BroadcasterConfig,
        cancel: CancellationToken,
    ) -> anyhow::Result<(Sender<SlotMessage>, Broadcaster)> {
        let (sender, mut receiver) = mpsc::channel(config.channel_capacity);

        let Ok(socket) = UdpSocket::bind(config.bind_address).await else {
            anyhow::bail!("failed to bind to address {}", config.bind_address);
        };

        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            loop {
                select! {
                    Some(message) = receiver.recv() => {
                        let Ok(data) = serde_json::to_vec(&message) else {
                            log::error!("failed to serialize message: {:?}", message);
                            continue;
                        };
                        if let Err(e) = socket.send_to(&data, &config.target_address).await {
                            log::error!("failed to send UDP packet: {e}");
                        }
                    }
                    _ = cancel.cancelled() => {
                        log::info!("broadcaster service is shutting down");
                        break;
                    }
                }
            }
            log::info!("Broadcaster service has shut down");
            Ok(())
        });

        Ok((
            sender,
            Broadcaster {
                handle,
                cancel: cancel_clone,
            },
        ))
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.cancel.cancel();
        self.handle.await??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agave_geyser_plugin_interface::geyser_plugin_interface::SlotStatus;
    use solana_time_utils::timestamp;
    use tokio::{
        net::UdpSocket,
        time::{timeout, Duration},
    };
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn test_broadcaster_sends_udp_messages() {
        let target_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();

        let config = BroadcasterConfig {
            bind_address: "127.0.0.1:0".parse().unwrap(),
            target_address: target_socket.local_addr().unwrap(),
            channel_capacity: 10,
        };

        let cancel = CancellationToken::new();
        let (sender, broadcaster) = Broadcaster::run(config, cancel.clone()).await.unwrap();

        // Act: send a message
        let msg = SlotMessage {
            slot: 1,
            status: SlotStatus::Completed,
            parent: None,
            dead_error: None,
            created_at: timestamp(),
        };
        let expected_msg = msg.clone();
        sender.try_send(msg).unwrap();

        // Assert: ensure UDP packet is received
        let mut buf = [0u8; 1024];
        let received = timeout(Duration::from_secs(1), target_socket.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();
        let data = &buf[..received.0];
        let actual_msg: SlotMessage = serde_json::from_slice(data).unwrap();

        assert_eq!(actual_msg, expected_msg);

        broadcaster.shutdown().await.unwrap();
    }
}
