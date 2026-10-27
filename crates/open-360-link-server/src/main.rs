use clap::{Parser, ValueEnum};
use open_360_link_protocol::{
    CameraModel, Insta360CameraController, NusbTransport, UvcControlTransport,
};
use open_360_link_server::{build_router, new_mock_state, AppState, ServerTransport};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Parser, Debug)]
#[command(
    name = "open-360-link-server",
    version,
    about = "Open 360 Link: Cross-platform HTTP REST & WebSocket remote control server for Insta360 Link cameras"
)]
struct Cli {
    /// Bind address for HTTP and WebSocket server
    #[arg(long, default_value = "127.0.0.1:9360", value_name = "ADDR")]
    bind: SocketAddr,

    /// Run against an in-memory simulated camera model instead of physical USB
    #[arg(long, value_enum, value_name = "MODEL")]
    mock: Option<MockModelArg>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MockModelArg {
    #[value(name = "link", alias = "link1")]
    Link,
    #[value(name = "link2")]
    Link2,
    #[value(name = "link2c")]
    Link2c,
}

impl From<MockModelArg> for CameraModel {
    fn from(val: MockModelArg) -> Self {
        match val {
            MockModelArg::Link => CameraModel::Link,
            MockModelArg::Link2 => CameraModel::Link2,
            MockModelArg::Link2c => CameraModel::Link2C,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "open_360_link_server=info,tower_http=info".into()),
        )
        .init();

    let cli = Cli::parse();

    let state: AppState = match cli.mock {
        Some(mock_arg) => {
            let model: CameraModel = mock_arg.into();
            tracing::info!(
                "Initializing in-memory simulated camera: {} (PID: 0x{:04x})",
                model.display_name(),
                model.pid()
            );
            new_mock_state(model)
        }
        None => {
            tracing::info!("Discovering and connecting to physical Insta360 Link camera via USB...");
            match NusbTransport::open_first() {
                Ok(usb) => {
                    let info = usb.camera_info().clone();
                    tracing::info!(
                        "Connected to physical camera: {} (Bus {} Addr {} • PID 0x{:04x})",
                        info.name,
                        info.bus_number,
                        info.device_address,
                        info.product_id
                    );
                    let ctrl = Insta360CameraController::new(ServerTransport::Usb(usb));
                    Arc::new(Mutex::new(ctrl))
                }
                Err(e) => {
                    tracing::warn!(
                        "Could not open physical Insta360 Link camera: {}. Falling back to simulated Link 2 camera. Pass '--mock <link|link2|link2c>' to select a specific model.",
                        e
                    );
                    new_mock_state(CameraModel::Link2)
                }
            }
        }
    };

    let router = build_router(state);

    let listener = tokio::net::TcpListener::bind(cli.bind).await?;
    tracing::info!("Insta360 Link Server listening on http://{}", cli.bind);
    tracing::info!("Web Remote UI:     http://{}", cli.bind);
    tracing::info!("WebSocket stream:  ws://{}/ws", cli.bind);
    tracing::info!("REST State API:    http://{}/api/state", cli.bind);

    axum::serve(listener, router).await?;

    Ok(())
}
