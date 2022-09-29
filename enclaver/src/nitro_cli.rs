use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

pub struct NitroCLI {
    program: String,
}

impl NitroCLI {
    pub fn new() -> Self {
        Self {
            program: String::from("nitro-cli"),
        }
    }

    pub async fn run_and_deserialize_output<T>(&self, args: impl NitroCLIArgs) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let child = Command::new(&self.program)
            .args(args.to_args()?)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| anyhow!("failed to execute nitro-cli: {}", err))?;

        let output = child.wait_with_output().await?;

        if output.status.success() {
            Ok(serde_json::from_slice(&output.stdout)?)
        } else {
            Err(anyhow!(
                "nitro-cli failed: {}",
                String::from_utf8(output.stderr)?
            ))
        }
    }

    pub async fn run_enclave(&self, args: RunEnclaveArgs) -> Result<EnclaveInfo> {
        self.run_and_deserialize_output(args).await
    }

    pub async fn run_enclave_with_debug(&self, args: RunEnclaveArgs) -> Result<()> {
        let mut cmd_args = args.to_args()?;
        cmd_args.push("--debug-mode".into());
        cmd_args.push("--attach-console".into());

        let exit_status = Command::new(&self.program)
            .args(cmd_args)
            .spawn()
            .map_err(|err| anyhow!("failed to execute nitro-cli: {}", err))?
            .wait()
            .await?;

        match exit_status.success() {
            true => Ok(()),
            false => Err(anyhow!("nitro-cli failed")),
        }
    }

    pub async fn describe_enclaves(&self) -> Result<Vec<EnclaveInfo>> {
        self
            .run_and_deserialize_output(DescribeEnclavesArgs {})
            .await
    }

    pub async fn terminate_enclave(&self, enclave_id: &str) -> Result<()> {
        let res: EnclaveTerminationStatus = self
            .run_and_deserialize_output(TerminateEnclaveArgs {
                enclave_id: enclave_id.to_string(),
            })
            .await?;

        match res.terminated {
            true => Ok(()),
            false => Err(anyhow!("nitro-cli failed to terminate enclave")),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EIFInfo {
    #[serde(rename = "Measurements")]
    measurements: EIFMeasurements,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EIFMeasurements {
    #[serde(rename = "PCR0")]
    pcr0: String,

    #[serde(rename = "PCR1")]
    pcr1: String,

    #[serde(rename = "PCR2")]
    pcr2: String,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EnclaveInfo {
    #[serde(rename = "EnclaveName")]
    pub name: String,

    #[serde(rename = "EnclaveID")]
    pub id: String,

    #[serde(rename = "ProcessID")]
    pub process_id: i32,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EnclaveTerminationStatus {
    #[serde(rename = "EnclaveID")]
    pub id: String,

    #[serde(rename = "Terminated")]
    pub terminated: bool,
}

pub trait NitroCLIArgs {
    fn to_args(&self) -> Result<Vec<OsString>>;
}

pub struct RunEnclaveArgs {
    pub cpu_count: i32,
    pub memory_mb: i32,
    pub eif_path: PathBuf,
    pub cid: Option<i32>,
}

impl NitroCLIArgs for RunEnclaveArgs {
    fn to_args(&self) -> Result<Vec<OsString>> {
        let mut args = vec![OsString::from("run-enclave")];

        if self.cpu_count < 1 {
            return Err(anyhow!(
                "at least 1 CPU is required, got: {}",
                self.cpu_count
            ));
        } else {
            args.push("--cpu-count".into());
            args.push(format!("{}", self.cpu_count).into());
        }

        if self.memory_mb < 64 {
            return Err(anyhow!(
                "at least 64MiB of memory are required, got: {}",
                self.memory_mb
            ));
        } else {
            args.push("--memory".into());
            args.push(format!("{}", self.memory_mb).into());
        }

        args.push("--eif-path".into());
        args.push(self.eif_path.clone().into());

        if let Some(cid) = self.cid {
            args.push("--enclave-cid".into());
            args.push(format!("{}", cid).into());
        }

        Ok(args)
    }
}

pub struct DescribeEnclavesArgs {}

impl NitroCLIArgs for DescribeEnclavesArgs {
    fn to_args(&self) -> Result<Vec<OsString>> {
        Ok(vec![OsString::from("describe-enclaves")])
    }
}

pub struct TerminateEnclaveArgs {
    pub enclave_id: String,
}

impl NitroCLIArgs for TerminateEnclaveArgs {
    fn to_args(&self) -> Result<Vec<OsString>> {
        Ok(vec![
            OsString::from("terminate-enclave"),
            OsString::from("--enclave-id"),
            OsString::from(&self.enclave_id),
        ])
    }
}
