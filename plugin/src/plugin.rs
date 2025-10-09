use {
    crate::{broadcaster::Broadcaster, config::Config, message::SlotMessage},
    agave_geyser_plugin_interface::geyser_plugin_interface::{
        GeyserPlugin, GeyserPluginError, Result as PluginResult, SlotStatus,
    },
    std::time::{Duration, Instant},
    tokio::{
        runtime::{Builder, Runtime},
        sync::mpsc::Sender,
        time::timeout,
    },
    tokio_util::sync::CancellationToken,
};

#[derive(Debug)]
pub struct PluginInner {
    runtime: Runtime,
    broadcaster_sender: Sender<SlotMessage>,
    broadcaster_service: Broadcaster,
    plugin_cancellation_token: CancellationToken,
}

impl PluginInner {
    fn send_message(&self, message: SlotMessage) -> anyhow::Result<()> {
        self.broadcaster_sender.try_send(message)?;
        Ok(())
    }
}

#[derive(Default, Debug)]
pub struct Plugin {
    inner: Option<PluginInner>,
}

impl Plugin {
    fn with_inner<F>(&self, f: F) -> PluginResult<()>
    where
        F: FnOnce(&PluginInner) -> PluginResult<()>,
    {
        let inner = self.inner.as_ref().expect("initialized");
        f(inner)
    }
}

impl GeyserPlugin for Plugin {
    fn name(&self) -> &'static str {
        concat!(
            env!("CARGO_PKG_NAME"),
            "-",
            env!("CARGO_PKG_VERSION"),
            "+",
            env!("GIT_VERSION")
        )
    }

    fn on_load(&mut self, config_file: &str, _is_reload: bool) -> PluginResult<()> {
        let config = Config::load_from_file(config_file)?;

        // Setup logger
        solana_logger::setup_with_default(&config.log.level);

        log::info!("loading plugin: {}", self.name());

        // Setup runtime
        let mut builder = Builder::new_multi_thread();
        builder.worker_threads(config.tokio.worker_threads);
        let plugin_cancellation_token = CancellationToken::new();

        let runtime = builder
            .thread_name(config.tokio.thread_name.clone())
            .enable_all()
            .build()
            .map_err(|error| GeyserPluginError::Custom(Box::new(error)))?;

        let plugin_cancellation_token_clone = plugin_cancellation_token.clone();
        let result = runtime.block_on(async move {
            let (broadcaster_sender, broadcaster_service) =
                Broadcaster::run(config.broadcaster, plugin_cancellation_token_clone)
                    .await
                    .map_err(|error| GeyserPluginError::Custom(format!("{error:?}").into()))?;
            Ok::<_, GeyserPluginError>((broadcaster_sender, broadcaster_service))
        });

        let (broadcaster_sender, broadcaster_service) = result.inspect_err(|e| {
            log::error!("failed to start plugin services: {e}");
            plugin_cancellation_token.cancel();
        })?;

        self.inner = Some(PluginInner {
            runtime,
            broadcaster_sender,
            broadcaster_service,
            plugin_cancellation_token,
        });

        Ok(())
    }

    fn on_unload(&mut self) {
        const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);
        if let Some(inner) = self.inner.take() {
            log::info!("shutting down plugin.");
            inner.plugin_cancellation_token.cancel();
            let now = Instant::now();
            log::info!(
                "waiting up to {:?} for plugin tasks to shut down",
                SHUTDOWN_TIMEOUT
            );
            let res = inner.runtime.block_on(async move {
                match timeout(SHUTDOWN_TIMEOUT, inner.broadcaster_service.shutdown()).await {
                    Ok(result) => result,
                    Err(_) => {
                        log::error!("shutdown timed out after {:?}", SHUTDOWN_TIMEOUT);
                        Err(anyhow::anyhow!("shutdown timed out"))
                    }
                }
            });

            if let Err(e) = res {
                log::error!("failed to shutdown broadcaster service: {e}");
            }

            inner.runtime.shutdown_timeout(SHUTDOWN_TIMEOUT);
            log::info!("tokio runtime shut down in {:?}", now.elapsed());
        }
    }

    fn update_slot_status(
        &self,
        slot: u64,
        parent: Option<u64>,
        status: &SlotStatus,
    ) -> PluginResult<()> {
        self.with_inner(|inner| {
            let message = SlotMessage::from_geyser(slot, parent, status);
            inner
                .send_message(message)
                .map_err(|e| GeyserPluginError::SlotStatusUpdateError { msg: e.to_string() })?;
            Ok(())
        })
    }

    fn account_data_notifications_enabled(&self) -> bool {
        false
    }

    fn account_data_snapshot_notifications_enabled(&self) -> bool {
        false
    }

    fn transaction_notifications_enabled(&self) -> bool {
        false
    }

    fn entry_notifications_enabled(&self) -> bool {
        false
    }
}
