# Slot Update Geyser plugin

This library listens to the geyser's slot related signals and broadcasts these
signals to specified host using UDP. It is to be used for the slot-latency
measuring, the minimalistic implementation has chosen over using yellostone gRPC
for the simplicity of the analysis and minimizing the added overhead.

## Run solana validator with plugin

```bash
$ solana-validator --geyser-plugin-config config.json
```

