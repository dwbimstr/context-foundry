use futures_util::StreamExt;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    model::{CallToolRequestParams, ServerCapabilities, ServerConfig},
    service::RxJsonRpcMessage,
    tool, tool_handler, tool_router,
    transport::{TokioChildProcess, async_rw::JsonRpcMessageCodec},
};
use tokio_util::codec::{Decoder, FramedRead, FramedWrite};
#[derive(Clone)]
struct Probe {
    tool_router: ToolRouter<Self>,
}
#[tool_router]
impl Probe {
    #[tool(description = "Synthetic deterministic reply")]
    fn echo(&self) -> String {
        "synthetic-ok".into()
    }
}
#[tool_handler]
impl ServerHandler for Probe {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("server") {
        let read = FramedRead::new(
            tokio::io::stdin(),
            JsonRpcMessageCodec::new_with_max_length(65536),
        )
        .scan((), |_, result| {
            std::future::ready(match result {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("frame_refused: {e}");
                    None
                }
            })
        });
        let write = FramedWrite::new(
            tokio::io::stdout(),
            JsonRpcMessageCodec::new_with_max_length(65536),
        );
        Probe {
            tool_router: Probe::tool_router(),
        }
        .serve((write, read))
        .await?
        .waiting()
        .await?;
        return Ok(());
    }
    let mut cmd = tokio::process::Command::new(std::env::current_exe()?);
    cmd.arg("server");
    let client = ().serve(TokioChildProcess::new(cmd)?).await?;
    let listed = client.list_tools(None).await?;
    anyhow::ensure!(listed.tools.len() == 1);
    let result = client.call_tool(CallToolRequestParams::new("echo")).await?;
    let rendered = serde_json::to_string(&result)?;
    anyhow::ensure!(rendered.contains("synthetic-ok"));
    client.cancel().await?;
    let mut codec = JsonRpcMessageCodec::<RxJsonRpcMessage<RoleServer>>::new_with_max_length(65536);
    let mut input = bytes::BytesMut::from(vec![b'x'; 65537].as_slice());
    anyhow::ensure!(
        codec.decode(&mut input).is_err(),
        "oversize predecode was accepted"
    );
    println!(
        "{}",
        serde_json::json!({"rmcp":"3.5.0","real_stdio_child":true,"initialize_list_call_close":true,"predecode_65537_rejected":true,"production_handler_backpressure":"not tested"})
    );
    Ok(())
}
