use crate::types::*;
use clap::Parser;
use clio::Input;
use std::str::FromStr;
use subxt::{
    ext::scale_value::{Composite, Value},
    OnlineClient, PolkadotConfig,
};
use subxt_signer::{sr25519::Keypair, SecretUri};

mod types;

#[subxt::subxt(runtime_metadata_path = "../etc/nh/metadata.scale")]
mod nh {}

/// Send a zksync-era proof to zkVerify chain
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// URL of the ws zkVerify endopoint
    #[arg(long, default_value_t = String::from("ws://localhost:9944"))]
    url: String,

    /// Seed phrase of the account
    #[arg(
        long,
        default_value_t = String::from("bottom drive obey lake curtain smoke basket hold race lonely fit walk//Alice")
    )]
    phrase: String,

    /// Binary file of the proof, use '-' for stdin
    #[clap(value_parser, default_value = "-")]
    proof: Input,
}

async fn run(
    url: String,
    seed_phrase: String,
    proof_path: Input,
) -> Result<(), Box<dyn std::error::Error>> {
    let api = OnlineClient::<PolkadotConfig>::from_url(url).await?;

    let proof = std::fs::read(proof_path.path().path())?;
    let parsed: L1BatchProofForL1 = bincode::deserialize(proof.as_slice())?;

    let (input_u256, proof_u256) = crypto_codegen::serialize_proof(&parsed.scheduler_proof);
    let proof_bytes = proof_u256
        .iter()
        .flat_map(u256_to_bytes_be)
        .collect::<Vec<u8>>();
    let input_bytes = input_u256
        .iter()
        .flat_map(u256_to_bytes_be)
        .collect::<Vec<u8>>();

    let uri = SecretUri::from_str(&seed_phrase)?;
    let keypair = Keypair::from_uri(&uri)?;

    let tx_payload = subxt::dynamic::tx(
        "SettlementZksyncPallet",
        "submit_proof",
        Composite::Named(vec![
            (
                "vk_or_hash".into(),
                Value::variant("Vk", Composite::Unnamed(vec![Value::from_bytes(vec![])])),
            ),
            ("proof".into(), Value::from_bytes(&proof_bytes)),
            ("pubs".into(), Value::from_bytes(&input_bytes)),
        ]),
    );

    let events = api
        .tx()
        .sign_and_submit_then_watch_default(&tx_payload, &keypair)
        .await?
        .wait_for_finalized_success()
        .await?;

    let new_element_event = events.find_first::<nh::poe::events::NewElement>()?.unwrap();
    println!("Proof successfully verified: {new_element_event:?}");

    Ok(())
}

#[tokio::main]
pub async fn main() {
    let args = Args::parse();

    if let Err(err) = run(args.url, args.phrase, args.proof).await {
        eprintln!("{err}");
    }
}
