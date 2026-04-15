use anyhow::Result;
use std::sync::Arc;

use super::*;
use super::models::*;

pub struct GitlabCIHandler {
    pub config: GitlabConfig,
}

#[derive(Debug, Clone)]
pub struct GitlabConfig {
    pub webhook_secret: String,
    pub auto_approve: bool,
}

impl GitlabCIHandler {
    pub fn new(webhook_secret: String, auto_approve: bool) -> Self {
        Self {
            config: GitlabConfig {
                webhook_secret,
                auto_approve,
            },
        }
    }
}

#[async_trait::async_trait]
impl CIHandler for GitlabCIHandler {
    fn name(&self) -> &str {
        "gitlab"
    }

    async fn handle(&self, payload: &str) -> Result<CIResponse> {
        let event = self.parse_webhook(payload)?;
        let headers = self.get_headers(payload);

        match event.object_kind.as_str() {
            "push" => self.handle_push(&event).await,
            "merge_request" => self.handle_merge_request(&event).await,
            "note" => self.handle_note(&event).await,
            "pipeline" => self.handle_pipeline(&event).await,
            _ => {
                log::debug!("Unhandled GitLab event: {}", event.object_kind);
                Ok(CIResponse {
                    success: true,
                    message: format!("Event {} received", event.object_kind),
                    provider: "gitlab".to_string(),
                    event_id: event.object_kind.clone(),
                })
            }
        }
    }
}

impl GitlabCIHandler {
    fn parse_webhook(&self, payload: &str) -> Result<GitlabEvent> {
        let event: GitlabEvent = serde_json::from_str(payload)?;
        Ok(event)
    }

    fn get_headers(&self, payload: &str) -> std::collections::HashMap<String, String> {
        let mut headers = std::collections::HashMap::new();
        headers.insert("x-gitlab-token".to_string(), "".to_string());
        headers
    }

    async fn handle_push(&self, event: &GitlabEvent) -> Result<CIResponse> {
        let push = &event.data;
        
        log::info!("GitLab push: {} - {}", push.project.path_with_namespace, push.ref_name);
        
        // Trigger FlowLink deployment
        let trigger = DeploymentTrigger {
            provider: "gitlab".to_string(),
            repository: push.project.path_with_namespace.clone(),
            branch: push.ref_name.replace("refs/heads/", ""),
            commit_sha: push.after.clone(),
            author: push.user_name.clone(),
            message: push.push_data.message.clone(),
            timestamp: push.created_at,
            auto_approve: self.config.auto_approve,
        };

        log::info!("Triggering deployment: {} at {}", trigger.repository, trigger.commit_sha);

        Ok(CIResponse {
            success: true,
            message: format!("Deployment triggered for {} at {}", trigger.repository, trigger.commit_sha),
            provider: "gitlab".to_string(),
            event_id: event.id,
            data: serde_json::to_value(trigger)?,
        })
    }

    async fn handle_merge_request(&self, event: &GitlabEvent) -> Result<CIResponse> {
        let mr = &event.data;
        
        log::info!("GitLab MR: {} - {}", mr.object_attributes.iid, mr.object_attributes.title);
        
        // Trigger CI/CD for MR
        Ok(CIResponse {
            success: true,
            message: format!("MR #{} {} - {}", mr.object_attributes.iid, mr.object_attributes.action, mr.object_attributes.title),
            provider: "gitlab".to_string(),
            event_id: event.id,
        })
    }

    async fn handle_note(&self, event: &GitlabEvent) -> Result<CIResponse> {
        let note = &event.data;
        
        log::info!("GitLab note: {} on {}", note.object_attributes.note, note.noteable_type);
        
        // Handle comments/notes on MRs/commits
        Ok(CIResponse {
            success: true,
            message: format!("Note received on {} #{}", note.noteable_type, note.noteable_attributes.iid),
            provider: "gitlab".to_string(),
            event_id: event.id,
        })
    }

    async fn handle_pipeline(&self, event: &GitlabEvent) -> Result<CIResponse> {
        let pipeline = &event.data;
        
        log::info!("GitLab pipeline: {} - {} ({})", pipeline.project.path_with_namespace, pipeline.object_attributes.status, pipeline.object_attributes.id);
        
        // Handle CI pipeline events
        Ok(CIResponse {
            success: true,
            message: format!("Pipeline {} completed for project {}", pipeline.object_attributes.status, pipeline.project.path_with_namespace),
            provider: "gitlab".to_string(),
            event_id: event.id,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabEvent {
    pub object_kind: String,
    pub id: String,
    pub data: GitlabEventData,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabEventData {
    pub object_kind: String,
    pub repository: Option<GitlabRepository>,
    pub project: GitlabProject,
    pub user: GitlabUser,
    pub ref_name: String,
    pub before: String,
    pub after: String,
    pub push_data: GitlabPushData,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabRepository {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabProject {
    pub id: i32,
    pub path_with_namespace: String,
    pub name: String,
    pub description: String,
    pub web_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabUser {
    pub id: i32,
    pub name: String,
    pub username: String,
    pub email: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabPushData {
    pub user_id: i32,
    pub user_name: String,
    pub ref: String,
    pub before: String,
    pub after: String,
    pub commit: Option<GitlabCommit>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabCommit {
    pub id: String,
    pub message: String,
}

// Merge request specific data
#[derive(Debug, Clone, Deserialize)]
pub struct GitlabMergeRequestData {
    pub object_kind: String,
    pub user: GitlabUser,
    pub project: GitlabProject,
    pub object_attributes: GitlabMergeRequest,
    pub changes: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabMergeRequest {
    pub id: i32,
    pub iid: i32,
    pub title: String,
    pub description: String,
    pub state: String,
    pub action: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub merged_at: Option<i64>,
    pub target_branch: String,
    pub source_branch: String,
}

// Note specific data
#[derive(Debug, Clone, Deserialize)]
pub struct GitlabNoteData {
    pub object_kind: String,
    pub object_attributes: GitlabNoteAttributes,
    pub noteable_type: String,
    pub noteable_attributes: GitlabNoteableAttributes,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabNoteAttributes {
    pub id: i32,
    pub note: String,
    pub created_at: i64,
    pub author_id: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabNoteableAttributes {
    pub iid: i32,
    pub title: String,
}

// Pipeline specific data
#[derive(Debug, Clone, Deserialize)]
pub struct GitlabPipelineData {
    pub object_kind: String,
    pub object_attributes: GitlabPipelineAttributes,
    pub project: GitlabProject,
    pub user: GitlabUser,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabPipelineAttributes {
    pub id: i64,
    pub ref_name: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}