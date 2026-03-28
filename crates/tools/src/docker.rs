use bollard::Docker;
use bollard::container::{ListContainersOptions, InspectContainerOptions};
use bollard::image::ListImagesOptions;
use serde::Serialize;
use std::collections::HashMap;

/// Structured container info.
#[derive(Debug, Serialize, Clone)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    pub ports: Vec<PortBinding>,
    pub labels: HashMap<String, String>,
}

/// Port mapping.
#[derive(Debug, Serialize, Clone)]
pub struct PortBinding {
    pub container_port: u16,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_ip: Option<String>,
}

/// Structured image info.
#[derive(Debug, Serialize, Clone)]
pub struct ImageInfo {
    pub id: String,
    pub tags: Vec<String>,
    pub size: i64,
    pub created: i64,
}

/// Container inspection result.
#[derive(Debug, Serialize, Clone)]
pub struct ContainerInspection {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    pub network: serde_json::Value,
    pub mounts: serde_json::Value,
    pub config: serde_json::Value,
}

/// Container state.
#[derive(Debug, Serialize, Clone)]
pub struct ContainerState {
    pub status: String,
    pub running: bool,
    pub pid: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
}

/// List result wrapper.
#[derive(Debug, Serialize, Clone)]
pub struct DockerListResult<T> {
    pub items: Vec<T>,
    pub count: u64,
}

/// Connect to Docker daemon.
pub fn connect() -> Result<Docker, String> {
    Docker::connect_with_local_defaults()
        .map_err(|e| format!("failed to connect to Docker: {e}"))
}

/// List containers.
pub async fn list_containers(docker: &Docker, all: bool) -> Result<DockerListResult<ContainerInfo>, String> {
    let options = ListContainersOptions::<String> {
        all,
        ..Default::default()
    };

    let containers = docker.list_containers(Some(options))
        .await
        .map_err(|e| format!("docker list failed: {e}"))?;

    let items: Vec<ContainerInfo> = containers.iter().map(|c| {
        let ports = c.ports.as_ref().map(|pp| {
            pp.iter().map(|p| PortBinding {
                container_port: p.private_port,
                protocol: p.typ.as_ref().map(|t| format!("{t:?}")).unwrap_or_default(),
                host_port: p.public_port,
                host_ip: p.ip.clone(),
            }).collect()
        }).unwrap_or_default();

        ContainerInfo {
            id: c.id.clone().unwrap_or_default(),
            name: c.names.as_ref()
                .and_then(|n| n.first())
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_default(),
            image: c.image.clone().unwrap_or_default(),
            state: c.state.clone().unwrap_or_default(),
            status: c.status.clone().unwrap_or_default(),
            created: c.created.unwrap_or(0),
            ports,
            labels: c.labels.clone().unwrap_or_default(),
        }
    }).collect();

    let count = items.len() as u64;
    Ok(DockerListResult { items, count })
}

/// List images.
pub async fn list_images(docker: &Docker) -> Result<DockerListResult<ImageInfo>, String> {
    let options = ListImagesOptions::<String> {
        all: false,
        ..Default::default()
    };

    let images = docker.list_images(Some(options))
        .await
        .map_err(|e| format!("docker images failed: {e}"))?;

    let items: Vec<ImageInfo> = images.iter().map(|img| {
        ImageInfo {
            id: img.id.clone(),
            tags: img.repo_tags.clone(),
            size: img.size,
            created: img.created,
        }
    }).collect();

    let count = items.len() as u64;
    Ok(DockerListResult { items, count })
}

/// Inspect a container.
pub async fn inspect_container(docker: &Docker, id: &str) -> Result<ContainerInspection, String> {
    let info = docker.inspect_container(id, None::<InspectContainerOptions>)
        .await
        .map_err(|e| format!("docker inspect failed: {e}"))?;

    let state = info.state.as_ref();
    let container_state = ContainerState {
        status: state.and_then(|s| s.status.as_ref()).map(|s| format!("{s:?}")).unwrap_or_default(),
        running: state.and_then(|s| s.running).unwrap_or(false),
        pid: state.and_then(|s| s.pid).unwrap_or(0),
        started_at: state.and_then(|s| s.started_at.clone()),
        finished_at: state.and_then(|s| s.finished_at.clone()),
        exit_code: state.and_then(|s| s.exit_code),
    };

    let network = info.network_settings
        .as_ref()
        .map(|n| serde_json::to_value(n).unwrap_or_default())
        .unwrap_or_default();

    let mounts = info.mounts
        .as_ref()
        .map(|m| serde_json::to_value(m).unwrap_or_default())
        .unwrap_or_default();

    let config = info.config
        .as_ref()
        .map(|c| serde_json::to_value(c).unwrap_or_default())
        .unwrap_or_default();

    Ok(ContainerInspection {
        id: info.id.unwrap_or_default(),
        name: info.name.unwrap_or_default().trim_start_matches('/').to_string(),
        image: info.image.unwrap_or_default(),
        state: container_state,
        network,
        mounts,
        config,
    })
}
