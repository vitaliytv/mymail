//! Product boundary for installed n-plugin Components in mlmail.
//!
//! The manager accepts only raw WebAssembly Components with an embedded
//! `nitra.plugin-manifest/v1`. It records their immutable activation generation
//! in the generic n-plugin registry; OAuth credentials never enter plugin files.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use n_plugin_compatibility::GraphLifecycleState;
use n_plugin_oci::{DirectOciResolutionBackend, DEFAULT_REGISTRY};
use n_plugin_package::{inspect_component, ReleaseIdentity, WitExportRef};
use n_plugin_runtime::{ActivationCompiler, ActivationGeneration, ActivationRegistry};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex as AsyncMutex;

use crate::{
    auth::{self, state::AuthState, storage::SharedStorage},
    endpoints::Endpoints,
    gmail::{
        plugin_booking_finder::invoke_booking_finder, plugin_draft_helper::invoke_draft_helper,
        plugin_runtime::build_gmail_plugin_runtime,
    },
    plugin_context::{self, PluginContextCoordinator},
    plugin_contracts::{
        is_no_consent_runtime_interface, MlmailPluginContractRegistry,
        GMAIL_BOOKING_FINDER_INTERFACE, GMAIL_DRAFT_HELPER_INTERFACE,
    },
    plugin_dispatch::{dispatch_component_at, publish_context_at, require_dispatch_grants},
    plugin_grants::{grant_store_path, PluginGrantKey, PluginGrantStore},
    plugin_install::{
        action_preview, activation_policy_for_grants, preflight_resolved_graph_for_account,
        PluginActionPreview, PluginDeploymentLock, PluginInstallPreview,
    },
};

const GMAIL_WKG_LOCK: &str = include_str!("../wkg.lock");
const CAS_GC_GRACE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// One installed root Component shown by the Vue Plugin Manager.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    /// Exact immutable release selected at installation time.
    pub release: ReleaseIdentity,
    /// Product triggers declared by the embedded manifest.
    pub triggers: Vec<String>,
    /// Explicit user intent; disabled Components cannot be invoked.
    pub enabled: bool,
    /// Exact registry generation committed for this installed projection.
    pub activation_generation: u64,
    /// Exact deployment lock retained for offline graph reconstruction.
    pub deployment_lock: PluginDeploymentLock,
    /// Exact approved root and dependency capability grants for this release.
    pub grants: Vec<PluginGrantKey>,
    /// Product host imports used directly by the root Component.
    pub root_host_interfaces: Vec<String>,
}

/// Output-only product projection for one exact installed Component.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginDto {
    /// Exact immutable release selected at installation time.
    pub release: ReleaseIdentity,
    /// Exact typed triggers committed in this activation generation.
    pub triggers: Vec<String>,
    /// Product-owned actions derived from the typed contract registry.
    pub actions: Vec<PluginActionPreview>,
    /// Explicit user intent retained by the product index.
    pub enabled: bool,
    /// Durable runtime lifecycle for the root graph.
    pub lifecycle: GraphLifecycleState,
    /// Exact immutable activation generation used to derive this projection.
    pub activation_generation: u64,
    /// Exact dependency releases retained by this generation.
    pub dependencies: Vec<ReleaseIdentity>,
    /// Content-bound server-owned deployment lock.
    pub deployment_lock: PluginDeploymentLock,
}

/// Opaque user response to one exact server-generated grant requirement.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginGrantDecisionDto {
    /// Requirement identifier copied unchanged from installation preview.
    pub requirement_id: String,
    /// Explicit user choice; denied required capabilities prevent activation.
    pub allow: bool,
}

/// Exact confirmation of one previously previewed local Component.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallConfirmationDto {
    /// Local Component path selected by the native file picker.
    pub path: String,
    /// Digest-bound preview identifier returned by the backend.
    pub preview_id: String,
    /// Exact release the user reviewed before confirming.
    pub expected_release: ReleaseIdentity,
    /// Opaque decisions for the exact grant requirement identifiers.
    pub grants: Vec<PluginGrantDecisionDto>,
}

/// Application-scoped serialization boundary for plugin installation and recovery.
#[derive(Default)]
pub struct PluginInstallState {
    mutex: AsyncMutex<()>,
}

/// Result returned after the installed Draft Helper creates a native Gmail draft.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDraftActionDto {
    /// Opaque Gmail draft identifier returned by Gmail.
    pub draft_id: String,
    /// Exact Component release that initiated the operation.
    pub release: ReleaseIdentity,
    /// Exact activation generation that supplied Component bytes.
    pub generation: u64,
}

/// One Gmail message reference returned by the typed Booking Finder action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBookingMessageDto {
    /// Opaque Gmail message identifier.
    pub id: String,
    /// Opaque Gmail thread identifier when Gmail returned one.
    pub thread_id: Option<String>,
}

/// Typed Booking Finder result projected onto mlmail's public command surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBookingActionDto {
    /// Exact Gmail search query chosen by the typed guest.
    pub query: String,
    /// Typed Gmail message references returned by the guest.
    pub messages: Vec<PluginBookingMessageDto>,
    /// Exact Component release that initiated the operation.
    pub release: ReleaseIdentity,
    /// Exact activation generation that supplied Component bytes.
    pub generation: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct PluginIndex {
    pub(crate) next_generation: u64,
    pub(crate) plugins: Vec<InstalledPlugin>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingActivation {
    target_release: ReleaseIdentity,
    target_generation: u64,
    staged_index_file: String,
    deployment_lock: PluginDeploymentLock,
    grants: Vec<PluginGrantKey>,
}

fn app_data(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("app_data_dir: {error}"))
}

fn plugin_root(app_data: &Path) -> PathBuf {
    app_data.join("n-plugin")
}

fn oci_cache_path(app_data: &Path) -> PathBuf {
    plugin_root(app_data).join("oci").join("cache")
}

fn deployment_lock_relative(release: &ReleaseIdentity) -> anyhow::Result<String> {
    let digest = release
        .digest
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("plugin release digest must use sha256"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        anyhow::bail!("plugin release digest must contain 64 lowercase hexadecimal characters");
    }
    Ok(format!("locks/{digest}.n-plugin.lock"))
}

fn deployment_lock_path(app_data: &Path, release: &ReleaseIdentity) -> anyhow::Result<PathBuf> {
    Ok(plugin_root(app_data).join(deployment_lock_relative(release)?))
}

fn deployment_lock_metadata(
    app_data: &Path,
    release: &ReleaseIdentity,
) -> anyhow::Result<PluginDeploymentLock> {
    let relative_path = deployment_lock_relative(release)?;
    let source = fs::read(plugin_root(app_data).join(&relative_path))?;
    Ok(PluginDeploymentLock {
        relative_path,
        digest: format!("sha256:{:x}", Sha256::digest(source)),
    })
}

fn finalize_retirements_and_collect(registry: &ActivationRegistry) -> anyhow::Result<()> {
    for retirement in registry.pending_retirements()? {
        registry.finalize_retirement(&retirement)?;
    }
    registry.collect_cas_garbage(CAS_GC_GRACE)?;
    Ok(())
}

pub(crate) fn registry(app_data: &Path) -> anyhow::Result<ActivationRegistry> {
    let root = plugin_root(app_data);
    ActivationRegistry::open(root.join("registry.sqlite3"), root.join("cas"))
}

fn index_path(app_data: &Path) -> PathBuf {
    plugin_root(app_data).join("installed.json")
}

fn staged_index_path(app_data: &Path) -> PathBuf {
    plugin_root(app_data).join("installed.pending.json")
}

fn pending_activation_path(app_data: &Path) -> PathBuf {
    plugin_root(app_data).join("pending-activation.json")
}

pub(crate) fn load_index(app_data: &Path) -> anyhow::Result<PluginIndex> {
    let path = index_path(app_data);
    match fs::read(&path) {
        Ok(source) => Ok(serde_json::from_slice(&source)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PluginIndex::default()),
        Err(error) => Err(error.into()),
    }
}

fn write_index(app_data: &Path, index: &PluginIndex) -> anyhow::Result<()> {
    let path = index_path(app_data);
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("plugin index has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".installed-{}.tmp", std::process::id()));
    fs::write(&temporary, serde_json::to_vec_pretty(index)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn write_json_atomically(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("plugin state path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("plugin state path has no UTF-8 file name"))?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, path)?;
    Ok(())
}

fn load_pending_activation(app_data: &Path) -> anyhow::Result<Option<PendingActivation>> {
    match fs::read(pending_activation_path(app_data)) {
        Ok(source) => Ok(Some(serde_json::from_slice(&source)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn discard_pending_activation(app_data: &Path) -> anyhow::Result<()> {
    for path in [
        pending_activation_path(app_data),
        staged_index_path(app_data),
    ] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn ensure_gmail_wkg_lock(app_data: &Path) -> anyhow::Result<PathBuf> {
    let path = plugin_root(app_data).join("wkg.lock");
    if let Ok(existing) = fs::read_to_string(&path) {
        if existing == GMAIL_WKG_LOCK {
            return Ok(path);
        }
        anyhow::bail!("installed Gmail WKG lock differs from this mlmail release");
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Gmail WKG lock has no parent directory"))?;
    fs::create_dir_all(parent)?;
    fs::write(&path, GMAIL_WKG_LOCK)?;
    Ok(path)
}

async fn contract_registry(app_data: &Path) -> anyhow::Result<MlmailPluginContractRegistry> {
    let lock_path = ensure_gmail_wkg_lock(app_data)?;
    MlmailPluginContractRegistry::load(lock_path).await
}

async fn preflight_plugin_at(
    app_data: &Path,
    path: &Path,
    account_id: Option<&str>,
) -> anyhow::Result<PluginInstallPreview> {
    let component = fs::read(path)?;
    let embedded = inspect_component(&component)?;
    let contracts = contract_registry(app_data).await?;
    let lock_path = deployment_lock_path(app_data, &embedded.release)?;
    let graph = DirectOciResolutionBackend::online(DEFAULT_REGISTRY, oci_cache_path(app_data))?
        .collect_graph(&component, &lock_path)
        .await?;
    let deployment_lock = deployment_lock_metadata(app_data, &embedded.release)?;
    preflight_resolved_graph_for_account(
        &component,
        &graph,
        deployment_lock,
        &contracts,
        account_id,
    )
}

async fn confirm_plugin_at(
    app_data: &Path,
    confirmation: &PluginInstallConfirmationDto,
    account_id: &str,
) -> anyhow::Result<InstalledPlugin> {
    if account_id.trim().is_empty() {
        anyhow::bail!("authenticate a mail account before plugin consent");
    }
    reconcile_pending_at(app_data)?;
    let component = fs::read(&confirmation.path)?;
    let embedded = inspect_component(&component)?;
    if embedded.release != confirmation.expected_release {
        anyhow::bail!("installation preview is stale: plugin release changed after review");
    }
    let contracts = contract_registry(app_data).await?;
    let lock_path = deployment_lock_path(app_data, &embedded.release)?;
    let graph = DirectOciResolutionBackend::offline(DEFAULT_REGISTRY, oci_cache_path(app_data))?
        .collect_graph(&component, &lock_path)
        .await?;
    let deployment_lock = deployment_lock_metadata(app_data, &embedded.release)?;
    let preview = preflight_resolved_graph_for_account(
        &component,
        &graph,
        deployment_lock.clone(),
        &contracts,
        Some(account_id),
    )?;
    if preview.preview_id != confirmation.preview_id {
        anyhow::bail!("installation preview is stale; run preflight and review consent again");
    }
    if preview.release != confirmation.expected_release {
        anyhow::bail!("plugin release changed after installation preview");
    }
    if !preview.compatible {
        anyhow::bail!(
            "plugin is incompatible with this mlmail release: {}",
            preview
                .reason
                .as_deref()
                .unwrap_or("installation preflight did not provide a reason")
        );
    }
    let approved_grants = approved_grants(&preview, &confirmation.grants)?;
    if embedded.release != preview.release {
        anyhow::bail!("plugin release changed during installation preflight");
    }
    let mut index = load_index(app_data)?;
    let activation_registry = registry(app_data)?;
    let generation = activation_registry.reserve_generation()?;
    let plan = ActivationCompiler::new()?.compile(
        &graph,
        generation,
        &contracts.activation_host_inventory(),
        &contracts.trigger_inventory(),
    )?;
    let required_grants = preview
        .required_capabilities
        .iter()
        .map(|requirement| requirement.grant_key(preview.release.clone()))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let policy = activation_policy_for_grants(&plan, &graph, &required_grants, &approved_grants)?;
    let host_interfaces = plan
        .host_interfaces
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let root_component = plan
        .components
        .iter()
        .find(|component| component.release == graph.root)
        .ok_or_else(|| anyhow::anyhow!("activation plan is missing its exact root Component"))?;
    let mut root_host_interfaces = root_component
        .imports
        .iter()
        .filter(|interface| host_interfaces.contains(interface.as_str()))
        .map(|interface| {
            let identity = WitExportRef::parse(interface)?;
            Ok((identity, interface.clone()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?
        .into_iter()
        .filter(|(identity, _)| !is_no_consent_runtime_interface(identity))
        .map(|(_, interface)| interface)
        .collect::<Vec<_>>();
    root_host_interfaces.sort();
    root_host_interfaces.dedup();

    let installed = InstalledPlugin {
        release: embedded.release,
        triggers: graph
            .nodes
            .iter()
            .find(|node| node.release == graph.root)
            .expect("resolved graph contains its root node")
            .manifest
            .triggers
            .iter()
            .map(|trigger| trigger.as_str().to_owned())
            .collect(),
        enabled: true,
        activation_generation: generation.get(),
        deployment_lock: deployment_lock.clone(),
        grants: approved_grants.clone(),
        root_host_interfaces,
    };
    index
        .plugins
        .retain(|existing| existing.release.package != installed.release.package);
    index.plugins.push(installed.clone());
    index
        .plugins
        .sort_by(|left, right| left.release.package.cmp(&right.release.package));
    index.next_generation = index.next_generation.max(generation.get());
    let staged_path = staged_index_path(app_data);
    let pending = PendingActivation {
        target_release: installed.release.clone(),
        target_generation: generation.get(),
        staged_index_file: staged_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow::anyhow!("staged plugin index has no UTF-8 file name"))?
            .to_owned(),
        deployment_lock,
        grants: approved_grants,
    };
    write_json_atomically(&pending_activation_path(app_data), &pending)?;
    if let Err(error) = write_json_atomically(&staged_path, &index) {
        discard_pending_activation(app_data)?;
        return Err(error);
    }

    if let Err(error) = activation_registry.publish_with_policy(&plan, &graph, &policy) {
        discard_pending_activation(app_data)?;
        return Err(error);
    }
    reconcile_pending_at(app_data).map_err(|error| {
        anyhow::anyhow!(
            "activation committed, reconciliation pending for `{}` generation {}: {error:#}",
            installed.release.package,
            generation.get()
        )
    })?;
    Ok(installed)
}

fn approved_grants(
    preview: &PluginInstallPreview,
    decisions: &[PluginGrantDecisionDto],
) -> anyhow::Result<Vec<PluginGrantKey>> {
    let expected = preview
        .required_capabilities
        .iter()
        .map(|requirement| requirement.requirement_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut received = std::collections::BTreeMap::new();
    for decision in decisions {
        if received
            .insert(decision.requirement_id.as_str(), decision.allow)
            .is_some()
        {
            anyhow::bail!(
                "duplicate grant decision `{}` requires a new preview",
                decision.requirement_id
            );
        }
    }
    if received
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        != expected
    {
        anyhow::bail!("grant decisions differ from the exact installation preview");
    }
    if let Some((requirement, _)) = received.iter().find(|(_, allow)| !**allow) {
        anyhow::bail!("required capability `{requirement}` was not approved");
    }
    let mut grants = preview
        .required_capabilities
        .iter()
        .map(|requirement| requirement.grant_key(preview.release.clone()))
        .collect::<anyhow::Result<Vec<_>>>()?;
    grants.sort();
    grants.dedup();
    Ok(grants)
}

fn reconcile_activation_at(app_data: &Path) -> anyhow::Result<()> {
    let Some(pending) = load_pending_activation(app_data)? else {
        return Ok(());
    };
    let generation = ActivationGeneration::new(pending.target_generation)?;
    let registry = registry(app_data)?;
    let Some(active) = registry.active(&pending.target_release)? else {
        discard_pending_activation(app_data)?;
        return Ok(());
    };
    if active.root != pending.target_release || active.generation != generation {
        if active.generation.get() < pending.target_generation {
            discard_pending_activation(app_data)?;
            return Ok(());
        }
        anyhow::bail!(
            "a different newer activation is active while generation {} remains pending",
            pending.target_generation
        );
    }
    let staged_path = staged_index_path(app_data);
    let expected_staged_file = staged_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("static staged index file name is UTF-8");
    if pending.staged_index_file != expected_staged_file {
        anyhow::bail!("pending activation references an unexpected staged projection");
    }
    let index = match fs::read(&staged_path) {
        Ok(source) => {
            let index = serde_json::from_slice::<PluginIndex>(&source)?;
            fs::rename(&staged_path, index_path(app_data))?;
            index
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => load_index(app_data)?,
        Err(error) => return Err(error.into()),
    };
    let exact = index.plugins.iter().any(|plugin| {
        plugin.release == pending.target_release
            && plugin.activation_generation == pending.target_generation
            && plugin.deployment_lock == pending.deployment_lock
            && plugin.grants == pending.grants
    });
    if !exact || index.next_generation < pending.target_generation {
        anyhow::bail!("committed plugin projection does not match pending activation");
    }
    PluginGrantStore::open(grant_store_path(app_data))?.grant_all(pending.grants)?;
    publish_context_at(app_data)?;
    discard_pending_activation(app_data)
}

fn reconcile_pending_at(app_data: &Path) -> anyhow::Result<()> {
    reconcile_activation_at(app_data)?;
    let activation_registry = registry(app_data)?;
    let retirements = activation_registry.pending_retirements()?;
    if retirements.is_empty() {
        return Ok(());
    }
    let mut index = load_index(app_data)?;
    let previous_len = index.plugins.len();
    index.plugins.retain(|plugin| {
        !retirements.iter().any(|retirement| {
            plugin.release == *retirement.root()
                && plugin.activation_generation == retirement.generation().get()
        })
    });
    if index.plugins.len() != previous_len {
        write_index(app_data, &index)?;
    }
    publish_context_at(app_data)?;
    finalize_retirements_and_collect(&activation_registry)
}

fn current_account(state: &StdMutex<AuthState>) -> anyhow::Result<String> {
    state
        .lock()
        .map_err(|_| anyhow::anyhow!("authentication state lock is poisoned"))?
        .email
        .clone()
        .ok_or_else(|| anyhow::anyhow!("authenticate a mail account before plugin consent"))
}

fn installed_projection(
    app_data: &Path,
    index: &PluginIndex,
    contracts: &MlmailPluginContractRegistry,
) -> anyhow::Result<Vec<InstalledPluginDto>> {
    let registry = registry(app_data)?;
    index
        .plugins
        .iter()
        .map(|plugin| {
            let generation = ActivationGeneration::new(plugin.activation_generation)?;
            let stored = registry.generation(generation)?;
            if stored.root != plugin.release {
                anyhow::bail!(
                    "installed projection generation {} belongs to another exact release",
                    plugin.activation_generation
                );
            }
            let active = registry.active(&plugin.release)?.ok_or_else(|| {
                anyhow::anyhow!(
                    "installed release `{}` has no active activation generation",
                    plugin.release.digest
                )
            })?;
            if active.root != plugin.release || active.generation != generation {
                anyhow::bail!(
                    "installed release `{}` does not match its exact active generation",
                    plugin.release.digest
                );
            }
            let lifecycle = registry.graph_lifecycle(&plugin.release)?.state;
            let actions = stored
                .triggers
                .iter()
                .filter_map(|trigger| {
                    contracts
                        .action_for(trigger)
                        .map(|action| action_preview(action, trigger))
                })
                .collect();
            Ok(InstalledPluginDto {
                release: plugin.release.clone(),
                triggers: stored
                    .triggers
                    .iter()
                    .map(|trigger| trigger.as_str().to_owned())
                    .collect(),
                actions,
                enabled: plugin.enabled,
                lifecycle,
                activation_generation: plugin.activation_generation,
                dependencies: stored
                    .nodes
                    .iter()
                    .filter(|node| node.release != plugin.release)
                    .map(|node| node.release.clone())
                    .collect(),
                deployment_lock: plugin.deployment_lock.clone(),
            })
        })
        .collect()
}

/// Reconciles a registry-committed activation before the application accepts plugin work.
///
/// # Errors
///
/// Returns an error when pending state is corrupt or durable projection/context repair fails.
pub fn reconcile_pending_installs(app: &AppHandle) -> Result<(), String> {
    reconcile_pending_at(&app_data(app)?).map_err(|error| error.to_string())
}

/// Lists root Components installed through the n-plugin Component manager.
#[tauri::command]
pub async fn plugin_manager_list(
    app: AppHandle,
    install_state: State<'_, PluginInstallState>,
) -> Result<Vec<InstalledPluginDto>, String> {
    let _guard = install_state.mutex.lock().await;
    let app_data = app_data(&app)?;
    reconcile_pending_at(&app_data).map_err(|error| error.to_string())?;
    let index = load_index(&app_data).map_err(|error| error.to_string())?;
    let contracts = contract_registry(&app_data)
        .await
        .map_err(|error| error.to_string())?;
    installed_projection(&app_data, &index, &contracts).map_err(|error| error.to_string())
}

/// Returns a read-only compatibility and consent preview for one local Component.
#[tauri::command]
pub async fn plugin_manager_preflight(
    app: AppHandle,
    path: String,
    install_state: State<'_, PluginInstallState>,
    auth_state: State<'_, StdMutex<AuthState>>,
) -> Result<PluginInstallPreview, String> {
    let _guard = install_state.mutex.lock().await;
    let app_data = app_data(&app)?;
    reconcile_pending_at(&app_data).map_err(|error| error.to_string())?;
    let account = current_account(auth_state.inner()).map_err(|error| error.to_string())?;
    preflight_plugin_at(&app_data, Path::new(&path), Some(&account))
        .await
        .map_err(|error| error.to_string())
}

/// Confirms one exact preview and publishes the activation before durable reconciliation.
#[tauri::command]
pub async fn plugin_manager_confirm_install(
    app: AppHandle,
    confirmation: PluginInstallConfirmationDto,
    install_state: State<'_, PluginInstallState>,
    auth_state: State<'_, StdMutex<AuthState>>,
) -> Result<InstalledPlugin, String> {
    let _guard = install_state.mutex.lock().await;
    let account = current_account(auth_state.inner()).map_err(|error| error.to_string())?;
    confirm_plugin_at(&app_data(&app)?, &confirmation, &account)
        .await
        .map_err(|error| error.to_string())
}

/// Compatibility wrapper that accepts only Components requiring no new consent.
#[tauri::command]
pub async fn plugin_manager_install(
    app: AppHandle,
    path: String,
    install_state: State<'_, PluginInstallState>,
    auth_state: State<'_, StdMutex<AuthState>>,
) -> Result<InstalledPlugin, String> {
    let _guard = install_state.mutex.lock().await;
    let app_data = app_data(&app)?;
    reconcile_pending_at(&app_data).map_err(|error| error.to_string())?;
    let account = current_account(auth_state.inner()).map_err(|error| error.to_string())?;
    let preview = preflight_plugin_at(&app_data, Path::new(&path), Some(&account))
        .await
        .map_err(|error| error.to_string())?;
    if !preview.required_capabilities.is_empty() {
        return Err(
            "plugin requires explicit consent; use preflight and confirm installation".to_owned(),
        );
    }
    let confirmation = PluginInstallConfirmationDto {
        path,
        preview_id: preview.preview_id,
        expected_release: preview.release,
        grants: Vec::new(),
    };
    confirm_plugin_at(&app_data, &confirmation, &account)
        .await
        .map_err(|error| error.to_string())
}

/// Changes explicit user enablement without deleting the immutable Component generation.
#[tauri::command]
pub async fn plugin_manager_set_disabled(
    app: AppHandle,
    target: ReleaseIdentity,
    disabled: bool,
    install_state: State<'_, PluginInstallState>,
) -> Result<(), String> {
    let _guard = install_state.mutex.lock().await;
    let app_data = app_data(&app)?;
    reconcile_pending_at(&app_data).map_err(|error| error.to_string())?;
    let mut index = load_index(&app_data).map_err(|error| error.to_string())?;
    let plugin = index
        .plugins
        .iter_mut()
        .find(|plugin| plugin.release == target)
        .ok_or_else(|| format!("exact plugin release `{}` is not installed", target.digest))?;
    plugin.enabled = !disabled;
    let lifecycle = if disabled {
        GraphLifecycleState::manually_disabled()
    } else {
        GraphLifecycleState::active()
    };
    registry(&app_data)
        .and_then(|registry| registry.set_graph_lifecycle(&plugin.release, lifecycle))
        .map_err(|error| error.to_string())?;
    write_index(&app_data, &index).map_err(|error| error.to_string())?;
    publish_context_at(&app_data).map_err(|error| error.to_string())
}

/// Disables and forgets one installed root Component; unreachable CAS data is collected later.
#[tauri::command]
pub async fn plugin_manager_uninstall(
    app: AppHandle,
    target: ReleaseIdentity,
    install_state: State<'_, PluginInstallState>,
) -> Result<(), String> {
    let _guard = install_state.mutex.lock().await;
    let app_data = app_data(&app)?;
    reconcile_pending_at(&app_data).map_err(|error| error.to_string())?;
    let mut index = load_index(&app_data).map_err(|error| error.to_string())?;
    let position = index
        .plugins
        .iter()
        .position(|plugin| plugin.release == target)
        .ok_or_else(|| format!("exact plugin release `{}` is not installed", target.digest))?;
    let plugin = index.plugins[position].clone();
    let activation_registry = registry(&app_data).map_err(|error| error.to_string())?;
    let retirement = activation_registry
        .begin_retirement(&plugin.release)
        .map_err(|error| error.to_string())?;
    index.plugins.remove(position);
    write_index(&app_data, &index).map_err(|error| error.to_string())?;
    publish_context_at(&app_data).map_err(|error| error.to_string())?;
    activation_registry
        .finalize_retirement(&retirement)
        .map_err(|error| error.to_string())?;
    activation_registry
        .collect_cas_garbage(CAS_GC_GRACE)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Runs the enabled Draft Helper Component with the current account's app-owned OAuth token.
#[tauri::command]
pub async fn plugin_draft_helper_create(
    app: AppHandle,
    target: ReleaseIdentity,
    endpoints: State<'_, Endpoints>,
    storage: State<'_, SharedStorage>,
    state: State<'_, StdMutex<AuthState>>,
    install_state: State<'_, PluginInstallState>,
) -> Result<PluginDraftActionDto, String> {
    let app_data = app_data(&app)?;
    let _guard = install_state.mutex.lock().await;
    reconcile_pending_at(&app_data).map_err(|error| error.to_string())?;
    publish_context_at(&app_data).map_err(|error| error.to_string())?;
    let contracts = contract_registry(&app_data)
        .await
        .map_err(|error| error.to_string())?;
    let selection =
        dispatch_component_at(&app_data, &target, GMAIL_DRAFT_HELPER_INTERFACE, &contracts)
            .map_err(|error| error.to_string())?;
    let account = current_account(state.inner()).map_err(|error| error.to_string())?;
    require_dispatch_grants(&app_data, &selection, &contracts, &account)
        .map_err(|error| error.to_string())?;
    let activation_registry = registry(&app_data).map_err(|error| error.to_string())?;
    let generation =
        ActivationGeneration::new(selection.generation).map_err(|error| error.to_string())?;
    let lock_path = ensure_gmail_wkg_lock(&app_data).map_err(|error| error.to_string())?;
    let runtime = build_gmail_plugin_runtime(&lock_path, &app.package_info().version.to_string())
        .await
        .map_err(|error| error.to_string())?;
    let context = PluginContextCoordinator::start(plugin_context::context_database(&app_data))
        .await
        .map_err(|error| error.to_string())?;
    let action = context
        .run_plugin(&selection.context_id, &selection.release, || async {
            let token = auth::acquire_access_token(
                &endpoints.google_token,
                storage.inner().as_ref(),
                state.inner(),
            )
            .await?;
            invoke_draft_helper(
                &runtime,
                &activation_registry,
                generation,
                &selection.component,
                &endpoints.gmail_messages_list,
                token,
            )
            .await
        })
        .await
        .map_err(|error| error.to_string());
    if let Err(error) = context.shutdown().await {
        log::error!("Draft Helper context shutdown failed after emission boundary: {error}");
    }
    let draft = action?;
    Ok(PluginDraftActionDto {
        draft_id: draft.id,
        release: selection.release,
        generation: selection.generation,
    })
}

/// Runs one exact Booking Finder Component through the generated typed adapter.
#[tauri::command]
pub async fn plugin_booking_finder_find(
    app: AppHandle,
    target: ReleaseIdentity,
    endpoints: State<'_, Endpoints>,
    storage: State<'_, SharedStorage>,
    state: State<'_, StdMutex<AuthState>>,
    install_state: State<'_, PluginInstallState>,
) -> Result<PluginBookingActionDto, String> {
    let app_data = app_data(&app)?;
    let _guard = install_state.mutex.lock().await;
    reconcile_pending_at(&app_data).map_err(|error| error.to_string())?;
    publish_context_at(&app_data).map_err(|error| error.to_string())?;
    let contracts = contract_registry(&app_data)
        .await
        .map_err(|error| error.to_string())?;
    let selection = dispatch_component_at(
        &app_data,
        &target,
        GMAIL_BOOKING_FINDER_INTERFACE,
        &contracts,
    )
    .map_err(|error| error.to_string())?;
    let account = current_account(state.inner()).map_err(|error| error.to_string())?;
    require_dispatch_grants(&app_data, &selection, &contracts, &account)
        .map_err(|error| error.to_string())?;
    let activation_registry = registry(&app_data).map_err(|error| error.to_string())?;
    let generation =
        ActivationGeneration::new(selection.generation).map_err(|error| error.to_string())?;
    let lock_path = ensure_gmail_wkg_lock(&app_data).map_err(|error| error.to_string())?;
    let runtime = build_gmail_plugin_runtime(&lock_path, &app.package_info().version.to_string())
        .await
        .map_err(|error| error.to_string())?;
    let context = PluginContextCoordinator::start(plugin_context::context_database(&app_data))
        .await
        .map_err(|error| error.to_string())?;
    let action = context
        .run_plugin(&selection.context_id, &selection.release, || async {
            let token = auth::acquire_access_token(
                &endpoints.google_token,
                storage.inner().as_ref(),
                state.inner(),
            )
            .await?;
            invoke_booking_finder(
                &runtime,
                &activation_registry,
                generation,
                &selection.component,
                &endpoints.gmail_messages_list,
                token,
            )
            .await
        })
        .await
        .map_err(|error| error.to_string());
    if let Err(error) = context.shutdown().await {
        log::error!("Booking Finder context shutdown failed after emission boundary: {error}");
    }
    let results = action?;
    Ok(PluginBookingActionDto {
        query: results.query,
        messages: results
            .messages
            .into_iter()
            .map(|message| PluginBookingMessageDto {
                id: message.id,
                thread_id: message.thread_id,
            })
            .collect(),
        release: selection.release,
        generation: selection.generation,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use anyhow::Result;
    use n_plugin_compatibility::GraphLifecycleState;
    use n_plugin_oci::{OciLockEntry, OciPluginLock};
    use n_plugin_package::{embed_manifest, PluginManifest, ReleaseIdentity};
    use n_plugin_runtime::ActivationGeneration;

    use super::{
        confirm_plugin_at, contract_registry, deployment_lock_path, discard_pending_activation,
        installed_projection, load_index, oci_cache_path, pending_activation_path, plugin_root,
        preflight_plugin_at, reconcile_pending_at, registry, staged_index_path, write_index,
        write_json_atomically, InstalledPlugin, PendingActivation, PluginDeploymentLock,
        PluginGrantDecisionDto, PluginIndex, PluginInstallConfirmationDto,
    };
    use crate::{
        plugin_context::{self, PluginContextCoordinator},
        plugin_contracts::{
            GMAIL_BOOKING_FINDER_INTERFACE, GMAIL_DRAFTS_INTERFACE, GMAIL_DRAFT_HELPER_INTERFACE,
            GMAIL_SEARCH_INTERFACE,
        },
        plugin_dispatch::{
            dispatch_component_at, require_dispatch_grants, PluginDispatchSelection,
        },
        plugin_grants::{grant_store_path, PluginGrantKey, PluginGrantStore},
    };

    #[test]
    fn persists_exact_component_identity_and_user_enablement() {
        let temporary = tempfile::tempdir().expect("temporary plugin directory should open");
        let index = PluginIndex {
            next_generation: 3,
            plugins: vec![InstalledPlugin {
                release: ReleaseIdentity {
                    package: "nitra:draft-helper".to_owned(),
                    version: "0.1.0".to_owned(),
                    digest:
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_owned(),
                },
                triggers: vec!["nitra:gmail/draft-helper@0.1.0".to_owned()],
                enabled: false,
                activation_generation: 3,
                deployment_lock: test_deployment_lock(),
                grants: Vec::new(),
                root_host_interfaces: Vec::new(),
            }],
        };

        write_index(temporary.path(), &index).expect("index should persist");
        assert_eq!(
            load_index(temporary.path()).expect("index should reopen"),
            index
        );
    }

    #[tokio::test]
    async fn previews_a_local_component_without_activation_writes() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let manifest = PluginManifest::from_toml(&format!(
            r#"
schema = "nitra.plugin-manifest/v1"
publisher_id = "other"
package = "draft-helper"
version = "0.1.0"
triggers = ["{GMAIL_DRAFT_HELPER_INTERFACE}"]

[entrypoints]
create = "{GMAIL_DRAFT_HELPER_INTERFACE}"
"#,
        ))?;
        let raw = wat::parse_str(format!(
            r#"
(component
  (type $host-contract (instance))
  (import "{GMAIL_DRAFTS_INTERFACE}" (instance (type $host-contract)))
  (core module $module)
  (core instance $core (instantiate $module))
  (instance $api)
  (export "{GMAIL_DRAFT_HELPER_INTERFACE}" (instance $api))
)
"#,
        ))?;
        let path = temporary.path().join("draft-helper.n-plugin");
        fs::write(&path, embed_manifest(&raw, &manifest)?)?;

        let preview =
            preflight_plugin_at(temporary.path(), &path, Some("person@example.com")).await?;

        assert!(
            preview.compatible,
            "unexpected reason: {:?}",
            preview.reason
        );
        let root = plugin_root(temporary.path());
        for relative in [
            "registry.sqlite3",
            "cas",
            "installed.json",
            "context.sqlite3",
            ".n-plugin.lock",
        ] {
            assert!(!root.join(relative).exists(), "unexpected {relative}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn confirms_only_the_exact_account_scoped_preview() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("draft-helper.n-plugin");
        fs::write(&path, packaged_draft("other", "draft-helper", "0.1.0")?)?;
        let preview =
            preflight_plugin_at(temporary.path(), &path, Some("person@example.com")).await?;
        let confirmation = confirmation(&path, &preview);

        let installed =
            confirm_plugin_at(temporary.path(), &confirmation, "person@example.com").await?;

        assert_eq!(installed.release, preview.release);
        assert_eq!(installed.activation_generation, 1);
        let active = registry(temporary.path())?
            .active(&installed.release)?
            .expect("confirmed plugin should have an active generation");
        assert_eq!(active.root, installed.release);
        assert_eq!(active.generation.get(), installed.activation_generation);
        let contracts = contract_registry(temporary.path()).await?;
        let projection =
            installed_projection(temporary.path(), &load_index(temporary.path())?, &contracts)?;
        assert_eq!(projection.len(), 1);
        assert_eq!(projection[0].release, installed.release);
        assert_eq!(projection[0].actions[0].kind, "draft-helper-create");
        assert_eq!(projection[0].lifecycle, GraphLifecycleState::active());
        let key = preview.required_capabilities[0].grant_key(preview.release)?;
        assert!(PluginGrantStore::open(grant_store_path(temporary.path()))?.allows(&key));
        assert!(!pending_activation_path(temporary.path()).exists());
        Ok(())
    }

    #[tokio::test]
    async fn confirms_an_exact_dependency_graph_and_rejects_tampered_offline_cache() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let dependency = packaged_provider()?;
        let first_path = temporary.path().join("dependent-v1.n-plugin");
        let first_component = packaged_dependent_draft("0.1.0")?;
        fs::write(&first_path, &first_component)?;
        seed_dependency_lock(temporary.path(), &first_component, &dependency)?;

        let first_preview =
            preflight_plugin_at(temporary.path(), &first_path, Some("person@example.com")).await?;
        assert!(
            first_preview.compatible,
            "unexpected: {:?}",
            first_preview.reason
        );
        assert_eq!(first_preview.dependencies.len(), 1);
        assert_eq!(first_preview.required_capabilities.len(), 1);
        assert_eq!(
            first_preview.required_capabilities[0].subject.package,
            "third:provider"
        );
        let first = confirm_plugin_at(
            temporary.path(),
            &confirmation(&first_path, &first_preview),
            "person@example.com",
        )
        .await?;
        let active = registry(temporary.path())?
            .active(&first.release)?
            .expect("dependency graph should activate");
        assert_eq!(active.nodes.len(), 2);
        assert_eq!(active.edges.len(), 1);
        assert_eq!(active.policy.edges[&active.edges[0].id].grants.len(), 1);

        let second_path = temporary.path().join("dependent-v2.n-plugin");
        let second_component = packaged_dependent_draft("0.2.0")?;
        fs::write(&second_path, &second_component)?;
        let cache_component =
            seed_dependency_lock(temporary.path(), &second_component, &dependency)?;
        let second_preview =
            preflight_plugin_at(temporary.path(), &second_path, Some("person@example.com")).await?;
        fs::write(cache_component, b"tampered")?;

        confirm_plugin_at(
            temporary.path(),
            &confirmation(&second_path, &second_preview),
            "person@example.com",
        )
        .await
        .expect_err("offline confirmation must verify every cached dependency");
        let still_active = registry(temporary.path())?
            .active(&first.release)?
            .expect("failed replacement must preserve the prior generation");
        assert_eq!(still_active.root, first.release);
        assert_eq!(
            load_index(temporary.path())?.plugins[0].release,
            first.release
        );
        Ok(())
    }

    #[tokio::test]
    async fn dispatches_two_draft_helpers_by_exact_release_and_fails_closed() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let first_path = temporary.path().join("first.n-plugin");
        let second_path = temporary.path().join("second.n-plugin");
        let first_component = packaged_draft("alpha", "draft-helper", "0.1.0")?;
        let second_component = packaged_draft("zeta", "draft-helper", "0.1.0")?;
        fs::write(&first_path, &first_component)?;
        fs::write(&second_path, &second_component)?;

        let first_preview =
            preflight_plugin_at(temporary.path(), &first_path, Some("person@example.com")).await?;
        let first = confirm_plugin_at(
            temporary.path(),
            &confirmation(&first_path, &first_preview),
            "person@example.com",
        )
        .await?;
        let second_preview =
            preflight_plugin_at(temporary.path(), &second_path, Some("person@example.com")).await?;
        let second = confirm_plugin_at(
            temporary.path(),
            &confirmation(&second_path, &second_preview),
            "person@example.com",
        )
        .await?;
        let contracts = contract_registry(temporary.path()).await?;

        let selected_second = dispatch_component_at(
            temporary.path(),
            &second.release,
            GMAIL_DRAFT_HELPER_INTERFACE,
            &contracts,
        )?;
        let selected_first = dispatch_component_at(
            temporary.path(),
            &first.release,
            GMAIL_DRAFT_HELPER_INTERFACE,
            &contracts,
        )?;
        assert_eq!(selected_second.release, second.release);
        let activation_registry = registry(temporary.path())?;
        let second_generation = activation_registry
            .generation(ActivationGeneration::new(second.activation_generation)?)?;
        assert_eq!(
            selected_second.component,
            activation_registry
                .cas()
                .read(&second_generation.composed_digest)?
        );
        assert_eq!(selected_first.release, first.release);
        let first_generation = activation_registry
            .generation(ActivationGeneration::new(first.activation_generation)?)?;
        assert_eq!(
            selected_first.component,
            activation_registry
                .cas()
                .read(&first_generation.composed_digest)?
        );
        assert_ne!(selected_first.component, selected_second.component);
        assert_ne!(selected_first.context_id, selected_second.context_id);
        require_dispatch_grants(
            temporary.path(),
            &selected_first,
            &contracts,
            "person@example.com",
        )?;
        let denied = require_dispatch_grants(
            temporary.path(),
            &selected_first,
            &contracts,
            "other@example.com",
        )
        .expect_err("exact Draft grant must not cross account boundaries");
        assert!(denied.to_string().contains("grant-required"));

        let booking_selection = PluginDispatchSelection {
            release: second.release.clone(),
            component: Vec::new(),
            context_id: "plugin:zeta:booking-finder".to_owned(),
            generation: second.activation_generation,
            host_interfaces: vec![GMAIL_SEARCH_INTERFACE.to_owned()],
            grants: Vec::new(),
            root_host_interfaces: vec![GMAIL_SEARCH_INTERFACE.to_owned()],
            edge_ids: Vec::new(),
        };
        let denied = require_dispatch_grants(
            temporary.path(),
            &booking_selection,
            &contracts,
            "person@example.com",
        )
        .expect_err("Booking search must fail closed without its exact generic grant");
        assert!(denied.to_string().contains("mail:search"));
        let mut grants = PluginGrantStore::open(grant_store_path(temporary.path()))?;
        grants.grant(PluginGrantKey::root_host(
            second.release.clone(),
            GMAIL_SEARCH_INTERFACE,
            "mail:search",
            "person@example.com",
        )?)?;
        require_dispatch_grants(
            temporary.path(),
            &booking_selection,
            &contracts,
            "person@example.com",
        )?;
        let wrong_digest = release(
            &first.release.package,
            &first.release.version,
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        );

        let context =
            PluginContextCoordinator::start(plugin_context::context_database(temporary.path()))
                .await?;
        assert_eq!(
            context
                .run_plugin(
                    &selected_second.context_id,
                    &selected_second.release,
                    || async { Ok("second") },
                )
                .await?,
            "second"
        );
        let mismatch = context
            .run_plugin(&selected_first.context_id, &wrong_digest, || async {
                Ok("must-not-run")
            })
            .await
            .expect_err("durable context must bind the package slot to the exact release");
        assert!(mismatch.to_string().contains("exact target release"));
        context.shutdown().await?;

        assert!(dispatch_component_at(
            temporary.path(),
            &first.release,
            GMAIL_BOOKING_FINDER_INTERFACE,
            &contracts,
        )
        .is_err());

        assert!(dispatch_component_at(
            temporary.path(),
            &wrong_digest,
            GMAIL_DRAFT_HELPER_INTERFACE,
            &contracts,
        )
        .is_err());

        let mut index = load_index(temporary.path())?;
        index
            .plugins
            .iter_mut()
            .find(|plugin| plugin.release == first.release)
            .expect("first exact release should remain installed")
            .enabled = false;
        write_index(temporary.path(), &index)?;
        assert!(dispatch_component_at(
            temporary.path(),
            &first.release,
            GMAIL_DRAFT_HELPER_INTERFACE,
            &contracts,
        )
        .is_err());

        let first_index = index
            .plugins
            .iter_mut()
            .find(|plugin| plugin.release == first.release)
            .expect("first exact release should remain installed");
        first_index.enabled = true;
        first_index.activation_generation += 1;
        write_index(temporary.path(), &index)?;
        assert!(dispatch_component_at(
            temporary.path(),
            &first.release,
            GMAIL_DRAFT_HELPER_INTERFACE,
            &contracts,
        )
        .is_err());

        index
            .plugins
            .retain(|plugin| plugin.release != second.release);
        write_index(temporary.path(), &index)?;
        assert!(dispatch_component_at(
            temporary.path(),
            &second.release,
            GMAIL_DRAFT_HELPER_INTERFACE,
            &contracts,
        )
        .is_err());
        Ok(())
    }

    #[tokio::test]
    async fn rejects_stale_preview_before_activation_publication() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("draft-helper.n-plugin");
        fs::write(&path, packaged_draft("other", "draft-helper", "0.1.0")?)?;
        let preview =
            preflight_plugin_at(temporary.path(), &path, Some("person@example.com")).await?;
        let confirmation = confirmation(&path, &preview);
        fs::write(&path, packaged_draft("other", "draft-helper", "0.2.0")?)?;

        let error = confirm_plugin_at(temporary.path(), &confirmation, "person@example.com")
            .await
            .expect_err("changed bytes must require a new preview");

        assert!(error.to_string().contains("preview is stale"));
        assert!(registry(temporary.path())?
            .active(&preview.release)?
            .is_none());
        assert!(load_index(temporary.path())?.plugins.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn rolls_forward_a_registry_committed_pending_projection() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("draft-helper.n-plugin");
        fs::write(&path, packaged_draft("other", "draft-helper", "0.1.0")?)?;
        let preview =
            preflight_plugin_at(temporary.path(), &path, Some("person@example.com")).await?;
        let confirmation = confirmation(&path, &preview);
        let installed =
            confirm_plugin_at(temporary.path(), &confirmation, "person@example.com").await?;
        let index = load_index(temporary.path())?;
        let staged = staged_index_path(temporary.path());
        write_json_atomically(&staged, &index)?;
        fs::remove_file(super::index_path(temporary.path()))?;
        let pending = PendingActivation {
            target_release: installed.release.clone(),
            target_generation: installed.activation_generation,
            staged_index_file: "installed.pending.json".to_owned(),
            deployment_lock: installed.deployment_lock.clone(),
            grants: preview
                .required_capabilities
                .iter()
                .map(|requirement| requirement.grant_key(preview.release.clone()))
                .collect::<Result<Vec<_>>>()?,
        };
        write_json_atomically(&pending_activation_path(temporary.path()), &pending)?;

        reconcile_pending_at(temporary.path())?;

        assert_eq!(load_index(temporary.path())?, index);
        assert!(!pending_activation_path(temporary.path()).exists());
        assert!(!staged.exists());
        Ok(())
    }

    #[test]
    fn discards_a_journal_that_never_reached_registry_commit() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let index = PluginIndex {
            next_generation: 1,
            plugins: vec![InstalledPlugin {
                release: release("other:draft-helper", "0.1.0", "sha256:pending"),
                triggers: vec![GMAIL_DRAFT_HELPER_INTERFACE.to_owned()],
                enabled: true,
                activation_generation: 1,
                deployment_lock: test_deployment_lock(),
                grants: Vec::new(),
                root_host_interfaces: Vec::new(),
            }],
        };
        write_json_atomically(&staged_index_path(temporary.path()), &index)?;
        write_json_atomically(
            &pending_activation_path(temporary.path()),
            &PendingActivation {
                target_release: index.plugins[0].release.clone(),
                target_generation: 1,
                staged_index_file: "installed.pending.json".to_owned(),
                deployment_lock: test_deployment_lock(),
                grants: Vec::new(),
            },
        )?;

        reconcile_pending_at(temporary.path())?;

        assert!(load_index(temporary.path())?.plugins.is_empty());
        assert!(!pending_activation_path(temporary.path()).exists());
        assert!(!staged_index_path(temporary.path()).exists());
        discard_pending_activation(temporary.path())?;
        Ok(())
    }

    fn confirmation(
        path: &Path,
        preview: &crate::plugin_install::PluginInstallPreview,
    ) -> PluginInstallConfirmationDto {
        PluginInstallConfirmationDto {
            path: path.display().to_string(),
            preview_id: preview.preview_id.clone(),
            expected_release: preview.release.clone(),
            grants: preview
                .required_capabilities
                .iter()
                .map(|requirement| PluginGrantDecisionDto {
                    requirement_id: requirement.requirement_id.clone(),
                    allow: true,
                })
                .collect(),
        }
    }

    fn test_deployment_lock() -> PluginDeploymentLock {
        PluginDeploymentLock {
            relative_path: "locks/test.n-plugin.lock".to_owned(),
            digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        }
    }

    fn packaged_draft(publisher: &str, package: &str, version: &str) -> Result<Vec<u8>> {
        let manifest = PluginManifest::from_toml(&format!(
            r#"
schema = "nitra.plugin-manifest/v1"
publisher_id = "{publisher}"
package = "{package}"
version = "{version}"
triggers = ["{GMAIL_DRAFT_HELPER_INTERFACE}"]

[entrypoints]
create = "{GMAIL_DRAFT_HELPER_INTERFACE}"
"#,
        ))?;
        let raw = wat::parse_str(format!(
            r#"
(component
  (type $host-contract (instance))
  (import "{GMAIL_DRAFTS_INTERFACE}" (instance (type $host-contract)))
  (core module $module)
  (core instance $core (instantiate $module))
  (instance $api)
  (export "{GMAIL_DRAFT_HELPER_INTERFACE}" (instance $api))
)
"#,
        ))?;
        Ok(embed_manifest(&raw, &manifest)?)
    }

    fn packaged_dependent_draft(version: &str) -> Result<Vec<u8>> {
        let provider = "third:provider/api@0.1.0";
        let manifest = PluginManifest::from_toml(&format!(
            r#"
schema = "nitra.plugin-manifest/v1"
publisher_id = "other"
package = "dependent"
version = "{version}"
triggers = ["{GMAIL_DRAFT_HELPER_INTERFACE}"]

[entrypoints]
create = "{GMAIL_DRAFT_HELPER_INTERFACE}"

[dependencies.provider]
package = "third:provider"
requirement = "=0.1.0"
imports = ["{provider}"]
"#,
        ))?;
        let raw = wat::parse_str(format!(
            r#"
(component
  (type $provider (instance))
  (import "{provider}" (instance (type $provider)))
  (core module $module)
  (core instance $core (instantiate $module))
  (instance $action)
  (export "{GMAIL_DRAFT_HELPER_INTERFACE}" (instance $action))
)
"#,
        ))?;
        Ok(embed_manifest(&raw, &manifest)?)
    }

    fn packaged_provider() -> Result<Vec<u8>> {
        let provider = "third:provider/api@0.1.0";
        let manifest = PluginManifest::from_toml(&format!(
            r#"
schema = "nitra.plugin-manifest/v1"
publisher_id = "third"
package = "provider"
version = "0.1.0"
triggers = []

[entrypoints]
provide = "{provider}"
"#,
        ))?;
        let raw = wat::parse_str(format!(
            r#"
(component
  (type $gmail (instance))
  (import "{GMAIL_DRAFTS_INTERFACE}" (instance (type $gmail)))
  (core module $module)
  (core instance $core (instantiate $module))
  (instance $api)
  (export "{provider}" (instance $api))
)
"#,
        ))?;
        Ok(embed_manifest(&raw, &manifest)?)
    }

    fn seed_dependency_lock(
        app_data: &Path,
        root_component: &[u8],
        dependency: &[u8],
    ) -> Result<std::path::PathBuf> {
        let root = n_plugin_package::inspect_component(root_component)?;
        let dependency_release = n_plugin_package::inspect_component(dependency)?;
        let mut lock = OciPluginLock {
            schema: "nitra.plugin-lock/v1".to_owned(),
            packages: vec![OciLockEntry {
                package: dependency_release.release.package.clone(),
                requirement: "=0.1.0".to_owned(),
                version: dependency_release.release.version.clone(),
                digest: dependency_release.release.digest.clone(),
                reference: "git.7n.ai/third/provider:0.1.0".to_owned(),
            }],
        };
        lock.write(&deployment_lock_path(app_data, &root.release)?)?;
        let hash = dependency_release
            .release
            .digest
            .strip_prefix("sha256:")
            .expect("inspected release digest uses sha256");
        let cache = oci_cache_path(app_data)
            .join("components")
            .join(format!("{hash}.n-plugin"));
        fs::create_dir_all(cache.parent().expect("cache Component has a parent"))?;
        fs::write(&cache, dependency)?;
        Ok(cache)
    }

    fn release(package: &str, version: &str, digest: &str) -> ReleaseIdentity {
        ReleaseIdentity {
            package: package.to_owned(),
            version: version.to_owned(),
            digest: digest.to_owned(),
        }
    }
}
