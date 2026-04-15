use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use chrono::{DateTime, Utc};

// Main webhook structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    pub id: String,
    pub service: String,
    pub data: String,
    pub timestamp: DateTime<Utc>,
    pub headers: HashMap<String, hyper::HeaderName>,
    pub ip_address: Option<String>,
}

// Webhook statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookStats {
    pub total_received: i64,
    pub total_routed: i64,
    pub total_failed: i64,
    pub service_stats: HashMap<String, ServiceStats>,
    pub last_received: Option<DateTime<Utc>>,
    pub uptime_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStats {
    pub received: i64,
    pub routed: i64,
    pub failed: i64,
    pub last_received: Option<DateTime<Utc>>,
}

// Configuration types
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WebhookReceiverConfig {
    pub port: u16,
    pub public_url: String,
    pub max_webhook_size: usize,
    pub retention_days: i32,
    pub allowed_origins: Vec<String>,
    pub hmac_secrets: Vec<WebhookHmacSecret>,
    pub enable_metrics: bool,
    pub enable_storage: bool,
    pub routing_rules: Vec<RoutingRule>,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WebhookHmacSecret {
    pub service: String,
    pub secret: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RoutingRule {
    pub service: String,
    pub enabled: bool,
    pub target: RoutingTarget,
    pub filters: Vec<RoutingFilter>,
    pub rate_limit: Option<RateLimit>,
    pub retry_policy: Option<RetryPolicy>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum RoutingTarget {
    FlowLink,
    Discord { channel: String },
    Slack { channel: String },
    Webhook { url: String },
    Local { handler: String },
    Email { to: Vec<String> },
    SMS { to: Vec<String> },
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RoutingFilter {
    pub field: String,
    pub operator: FilterOperator,
    pub value: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum FilterOperator {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Regex,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RateLimit {
    pub requests_per_minute: i32,
    pub requests_per_hour: i32,
    pub requests_per_day: i32,
    pub burst_size: i32,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RetryPolicy {
    pub max_retries: i32,
    pub delay_seconds: i32,
    pub backoff_multiplier: f64,
    pub max_delay_seconds: i32,
}

// Database and Redis configurations
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub pool_size: u32,
    pub max_lifetime: std::time::Duration,
    pub idle_timeout: std::time::Duration,
    pub enable_migrations: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout: std::time::Duration,
    pub read_timeout: std::time::Duration,
    pub write_timeout: std::time::Duration,
}

// Service-specific webhook models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubWebhook {
    pub action: String,
    pub repository: GithubRepository,
    pub sender: GithubUser,
    pub installation: Option<GithubInstallation>,
    pub organization: Option<GithubOrganization>,
    pub pull_request: Option<GithubPullRequest>,
    pub issue: Option<GithubIssue>,
    pub commit: Option<GithubCommit>,
    pub push_data: Option<GithubPushData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRepository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: String,
    pub url: String,
    pub html_url: String,
    pub clone_url: String,
    pub ssh_url: String,
    pub language: Option<String>,
    pub fork: bool,
    pub archived: bool,
    pub disabled: bool,
    pub private: bool,
    pub size: i64,
    pub stargazers_count: i32,
    pub watchers_count: i32,
    pub forks_count: i32,
    pub open_issues_count: i32,
    pub license: Option<GithubLicense>,
    pub topics: Vec<String>,
    pub has_issues: bool,
    pub has_projects: bool,
    pub has_wiki: bool,
    pub has_pages: bool,
    pub has_downloads: bool,
    pub has_discussions: bool,
    pub archived: bool,
    pub disabled: bool,
    pub visibility: String,
    pub pushed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubUser {
    pub id: i64,
    pub login: String,
    pub avatar_url: String,
    pub gravatar_id: String,
    pub url: String,
    pub html_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub gists_url: String,
    pub starred_url: String,
    pub subscriptions_url: String,
    pub organizations_url: String,
    pub repos_url: String,
    pub events_url: String,
    pub received_events_url: String,
    pub type: String,
    pub site_admin: bool,
    pub name: Option<String>,
    pub company: Option<String>,
    pub blog: Option<String>,
    pub location: Option<String>,
    pub email: Option<String>,
    pub hireable: Option<bool>,
    pub bio: Option<String>,
    pub twitter_username: Option<String>,
    pub public_repos: i32,
    pub public_gists: i32,
    pub followers: i32,
    pub following: i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubInstallation {
    pub id: i64,
    pub app_id: i64,
    pub account: GithubUser,
    pub repository_selection: String,
    pub access_tokens_url: String,
    pub repositories_url: String,
    pub html_url: String,
    pub app_slug: String,
    pub target_id: i64,
    pub target_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubOrganization {
    pub id: i64,
    pub login: String,
    pub avatar_url: String,
    pub gravatar_id: String,
    pub url: String,
    pub html_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub gists_url: String,
    pub starred_url: String,
    pub subscriptions_url: String,
    pub organizations_url: String,
    pub repos_url: String,
    pub events_url: String,
    pub received_events_url: String,
    pub type: String,
    pub site_admin: bool,
    pub name: Option<String>,
    pub company: Option<String>,
    pub blog: Option<String>,
    pub location: Option<String>,
    pub email: Option<String>,
    pub public_repos: i32,
    pub public_gists: i32,
    pub followers: i32,
    pub following: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubPullRequest {
    pub id: i64,
    pub number: i32,
    pub state: String,
    pub locked: bool,
    pub title: String,
    pub user: GithubUser,
    pub body: Option<String>,
    pub url: String,
    pub html_url: String,
    pub diff_url: Option<String>,
    pub patch_url: Option<String>,
    pub issue_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub merged_at: Option<DateTime<Utc>>,
    pub merge_commit_sha: Option<String>,
    pub assignee: Option<GithubUser>,
    pub assignees: Vec<GithubUser>,
    pub milestone: Option<GithubMilestone>,
    pub pull_request: GithubPullRequest,
    pub head: GithubBranch,
    pub base: GithubBranch,
    pub _links: GithubPullRequestLinks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubIssue {
    pub id: i64,
    pub number: i32,
    pub state: String,
    pub locked: bool,
    pub title: String,
    pub user: GithubUser,
    pub body: Option<String>,
    pub url: String,
    pub html_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub assignee: Option<GithubUser>,
    pub assignees: Vec<GithubUser>,
    pub milestone: Option<GithubMilestone>,
    pub pull_request: Option<GithubPullRequest>,
    pub labels: Vec<GithubLabel>,
    pub comments: i32,
    pub events: i32,
    pub performed_via_github_app: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubCommit {
    pub id: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub author: GithubUser,
    pub committer: GithubUser,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubPushData {
    pub before: String,
    pub after: String,
    pub ref: String,
    pub created: bool,
    pub deleted: bool,
    pub forced: bool,
    pub compare: String,
    pub total_commits: i32,
    pub commits: Vec<GithubCommit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubLicense {
    pub key: String,
    pub name: String,
    pub spdx_id: String,
    pub url: Option<String>,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubMilestone {
    pub id: i64,
    pub number: i32,
    pub state: String,
    pub title: String,
    pub description: Option<String>,
    pub due_on: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub url: String,
    pub html_url: String,
    pub labels: Vec<GithubLabel>,
    pub closed_issues: i32,
    pub open_issues: i32,
    pub assignees: Vec<GithubUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubLabel {
    pub id: i64,
    pub node_id: String,
    pub url: String,
    pub name: String,
    pub color: String,
    pub default: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubBranch {
    pub ref: String,
    pub sha: String,
    pub user: Option<GithubUser>,
    pub repo: Option<GithubRepository>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubPullRequestLinks {
    pub self_: String,
    pub html: String,
    pub issue: String,
    pub comments: String,
    pub review_comments: String,
    pub review_comment: String,
    pub comments_url: String,
    pub statuses: String,
}

// GitLab webhook models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabWebhook {
    pub object_kind: String,
    pub event_name: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub ref: Option<String>,
    pub checkout_sha: Option<String>,
    pub user: GitlabUser,
    pub project: GitlabProject,
    pub repository: Option<GitlabRepository>,
    pub commit: Option<GitlabCommit>,
    pub commits: Vec<GitlabCommit>,
    pub total_commits_count: i32,
    pub changes: Vec<GitlabChange>,
    pub merge_request: Option<GitlabMergeRequest>,
    pub issue: Option<GitlabIssue>,
    pub build: Option<GitlabBuild>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabUser {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabProject {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub web_url: String,
    pub avatar_url: Option<String>,
    pub git_ssh_url: String,
    pub git_http_url: String,
    pub namespace: String,
    pub visibility_level: i32,
    pub default_branch: String,
    pub homepage: Option<String>,
    pub url: String,
    pub ssh_url_to_repo: String,
    pub http_url_to_repo: String,
    pub readme_url: Option<String>,
    pub tag_list: Vec<String>,
    pub topics: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub creator_id: i64,
    pub group_with_project_visibility_level: Option<i32>,
    pub ci_config_path: Option<String>,
    pub public_jobs: bool,
    pub shared_with_groups: Option<Vec<String>>,
    pub allow_forking: bool,
    pub forks_count: i32,
    pub starred_by: Vec<String>,
    pub archived: bool,
    pub visibility: String,
    pub resolve_outdated_diff_discussions: bool,
    public_builds: bool,
    container_registry_enabled: bool,
    issues_enabled: bool,
    shared_runners_enabled: bool,
    lfs_enabled: bool,
    created_at: DateTime<Utc>,
    last_activity_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabRepository {
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub git_http_url: String,
    pub git_ssh_url: String,
    pub visibility_level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabCommit {
    pub id: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub author: GitlabUser,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabChange {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub diff: Option<String>,
    pub new_file: bool,
    pub renamed_file: bool,
    pub deleted_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabMergeRequest {
    pub id: i64,
    pub iid: i32,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub target_branch: String,
    pub source_branch: String,
    pub source_project_id: i64,
    pub target_project_id: i64,
    pub author: GitlabUser,
    pub assignee: Option<GitlabUser>,
    pub assignees: Vec<GitlabUser>,
    pub reviewers: Vec<GitlabUser>,
    pub milestone: Option<GitlabMilestone>,
    pub labels: Vec<String>,
    pub merge_when_pipeline_succeeds: bool,
    pub merge_status: String,
    pub merge_error: Option<String>,
    pub merged_by: Option<GitlabUser>,
    pub merged_at: Option<DateTime<Utc>>,
    pub closed_by: Option<GitlabUser>,
    pub closed_at: Option<DateTime<Utc>>,
    pub target_project_url: Option<String>,
    pub source_project_url: Option<String>,
    pub labels_url: String,
    pub discussions_url: String,
    pub should_remove_source_branch: bool,
    pub force_remove_source_branch: bool,
    pub work_in_progress: bool,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabIssue {
    pub id: i64,
    pub iid: i32,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub author: GitlabUser,
    pub assignee: Option<GitlabUser>,
    pub assignees: Vec<GitlabUser>,
    pub milestone: Option<GitlabMilestone>,
    pub labels: Vec<String>,
    pub project_id: i64,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabBuild {
    pub id: i64,
    pub stage: String,
    pub name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration: Option<i32>,
    pub allow_failure: bool,
    pub stage_idx: i32,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitlabMilestone {
    pub id: i64,
    pub iid: i32,
    pub title: String,
    pub description: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub state: String,
    pub expired: bool,
}