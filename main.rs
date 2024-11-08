use std::{fs::File, io::Write, time::Duration};

use clap::Parser;
use env_logger::{Builder, Env};
use log::{debug, error, info, trace};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_serial::SerialPortBuilderExt;

#[derive(Parser, Debug, Clone)]
pub(crate) struct Args {
    #[arg(short, long)]
    pub(crate) device: String,
    #[arg(short, long)]
    pub(crate) us: String,
    #[arg(short, long)]
    pub(crate) them: String,
    #[arg(short, long, default_value_t = 115_200)]
    pub(crate) baudrate: u32,
    #[arg(short = 'o', long, default_value_t = 1)]
    pub(crate) timeout: u64,
    #[arg(short, long, default_value_t = 30)]
    pub(crate) fec_threshold: u64,
    #[arg(short, long, default_value_t = 5)]
    pub(crate) retransmissions: u64,
    #[arg(short, long, default_value_t = 20)]
    pub(crate) channel_busy_threshold: u64,
    #[arg(short, long, default_value_t = false)]
    pub(crate) flood: bool,
    #[arg(short, long, default_value_t = 200)]
    pub(crate) flood_packet_size: usize,
    #[arg(short, long)]
    pub(crate) statistics_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::from_env(Env::default().default_filter_or("trace")).init();

    let args = Args::parse();

    debug!("{:?}", args);

    let mut port = tokio_serial::new(args.device.clone(), args.baudrate)
        .timeout(Duration::from_millis(args.timeout))
        .open_native_async()?;

    std::thread::sleep(Duration::from_millis(5000));

    port.write_fmt(format_args!("a[{}]\n", args.us))?;
    std::thread::sleep(Duration::from_millis(1000));
    port.write_fmt(format_args!("c[1,0,{}]\n", args.retransmissions))?;
    std::thread::sleep(Duration::from_millis(1000));
    port.write_fmt(format_args!("c[0,1,{}]\n", args.fec_threshold))?;
    std::thread::sleep(Duration::from_millis(1000));
    port.write_fmt(format_args!("c[0,2,{}]\n", args.channel_busy_threshold))?;
    std::thread::sleep(Duration::from_millis(1000));

    info!(
        "Successfully connected to and configured device {} with address {}",
        args.device, args.us
    );

    let (serial_reader, serial_writer) = tokio::io::split(port);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(200);
    let (statistics_tx, mut statistics_rx) = tokio::sync::mpsc::channel::<String>(1024);

    let statistics_task = {
        let args = args.clone();
        tokio::task::spawn(async move {
            if args.statistics_path.is_none() {
                return;
            }
            let mut writer = File::create(args.statistics_path.unwrap()).unwrap();
            while let Some(data) = statistics_rx.recv().await {
                if let Err(e) = writer.write_all(data.as_bytes()) {
                    error!("Error writing to statistics file: {}", e);
                    break;
                }
            }
        })
    };

    let reader = {
        let tx = tx.clone();
        let args = args.clone();
        tokio::task::spawn(async move {
            let mut reader = BufReader::new(serial_reader);
            let mut buffer = Vec::new();
            if args.flood {
                if tx
                    .send(
                        format!(
                            "m[{}\0,{}]\n",
                            "A".repeat(args.flood_packet_size),
                            args.them
                        )
                        .into_bytes(),
                    )
                    .await
                    .is_err()
                {
                    error!("Receiver dropped");
                    return;
                }
            }
            loop {
                buffer.clear();
                match reader.read_until(b'\n', &mut buffer).await {
                    Ok(0) => {
                        debug!("Serial port closed.");
                        break;
                    }
                    Ok(n) => {
                        let value = String::from_utf8_lossy(&buffer[..(n - 1)]);
                        trace!(
                            "Device {} with address {} received frame: {}",
                            args.device,
                            args.us,
                            value
                        );
                        if args.statistics_path.is_some() && value.starts_with("s") {
                            statistics_tx
                                .send(String::from_utf8_lossy(&buffer[..n]).to_string())
                                .await
                                .unwrap();
                        }
                        if args.flood && value.starts_with("m[D") {
                            let _ = tokio::time::sleep(Duration::from_millis(10)).await;
                            if tx
                                .send(
                                    format!(
                                        "m[{}\0,{}]\n",
                                        "A".repeat(args.flood_packet_size),
                                        args.them
                                    )
                                    .into_bytes(),
                                )
                                .await
                                .is_err()
                            {
                                error!("Receiver dropped");
                                break;
                            }
                        }
                        if value.starts_with("m[R,D") {
                            let value = String::from_utf8_lossy(&buffer[6..(n - 2)]);
                            info!(
                                "Device {} with address {} received message: {}",
                                args.device, args.us, value
                            );
                        }
                    }
                    Err(e) => {
                        error!("Error reading from serial port: {}", e);
                        break;
                    }
                }
            }
        })
    };

    let writer = {
        let tx = tx.clone();
        let args = args.clone();
        tokio::task::spawn(async move {
            let stdin = BufReader::new(tokio::io::stdin());
            let mut lines = stdin.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx
                    .send(format!("m[{}\0,{}]\n", line, args.them).into_bytes())
                    .await
                    .is_err()
                {
                    error!("Receiver dropped");
                    break;
                }
            }
        })
    };

    let serial_writer = tokio::task::spawn(async move {
        let mut writer = serial_writer;
        while let Some(data) = rx.recv().await {
            if let Err(e) = writer.write_all(&data).await {
                error!("Error writing to serial port: {}", e);
                break;
            }
        }
    });

    tokio::try_join!(reader, writer, serial_writer, statistics_task)?;

    Ok(())
}
