use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Frame,
    Layout, Margin, RichText, ScrollArea, Stroke, TextEdit, Vec2,
};
use tokio::runtime::Runtime;

use crate::db;
use crate::error::AppResult;
use crate::models::{
    IssueDetail, IssueView, NewIssue, NewLabel, NewUser, Project, UpdateIssue, User, Workspace,
};
use crate::repository::{IssueFilter, Repository};

const BG: Color32 = Color32::from_rgb(16, 17, 19);
const PANEL: Color32 = Color32::from_rgb(27, 28, 31);
const PANEL_HOVER: Color32 = Color32::from_rgb(39, 40, 44);
const BORDER: Color32 = Color32::from_rgb(57, 59, 64);
const TEXT: Color32 = Color32::from_rgb(239, 239, 241);
const MUTED: Color32 = Color32::from_rgb(171, 172, 176);
const FAINT: Color32 = Color32::from_rgb(120, 122, 127);
const ACCENT: Color32 = Color32::from_rgb(197, 199, 203);
const ACCENT_SOFT: Color32 = Color32::from_rgb(59, 61, 65);
const COLUMN_WIDTH: f32 = 224.0;
const CORNER_RADIUS: f32 = 12.0;
const UI_FONT: &str = "Clarity";
const CONTENT_FONT: &str = "Finlandica";
const CATEGORY_COLORS: [&str; 6] = [
    "#e5484d", "#5b8def", "#8a8f98", "#30a46c", "#b07cf7", "#d9a441",
];

const STATUSES: [(&str, &str); 5] = [
    ("backlog", "Backlog"),
    ("todo", "Todo"),
    ("in_progress", "In Progress"),
    ("done", "Done"),
    ("canceled", "Canceled"),
];

const PRIORITIES: [(i64, &str); 5] = [
    (0, "No priority"),
    (1, "Low"),
    (2, "Medium"),
    (3, "High"),
    (4, "Urgent"),
];

#[derive(Clone)]
struct NewIssueForm {
    title: String,
    description: String,
    due_date: String,
    status: String,
    priority: i64,
    estimate: Option<i64>,
    project_id: Option<String>,
    assignee_id: Option<String>,
    label_ids: Vec<String>,
}

impl NewIssueForm {
    fn new(default_status: &str, projects: &[Project], me: Option<&User>) -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            due_date: String::new(),
            status: default_status.to_owned(),
            priority: 0,
            estimate: None,
            project_id: projects.first().map(|project| project.id.clone()),
            assignee_id: me.map(|user| user.id.clone()),
            label_ids: Vec::new(),
        }
    }

    fn into_issue(self) -> NewIssue {
        NewIssue {
            title: self.title.trim().to_owned(),
            description: Some(self.description),
            status: Some(self.status),
            priority: Some(self.priority),
            project_id: self.project_id,
            assignee_id: self.assignee_id,
            label_ids: Some(self.label_ids),
            estimate: self.estimate,
            due_date: (!self.due_date.trim().is_empty()).then(|| self.due_date.trim().to_owned()),
        }
    }
}

enum Action {
    OpenIssue(String),
    CreateIssue(NewIssue),
    UpdateIssue(String, UpdateIssue),
    MoveIssue(String, String, f64),
    SetLabels(String, Vec<String>),
    DeleteIssue(String),
    AddComment(String, String),
    AddSubtask(String, String),
    ToggleSubtask(String),
}

enum WorkspaceAction {
    CreateUser(NewUser),
    UpdateUser(String, NewUser),
    SetCurrentUser(String),
    DeleteUser(String),
    CreateLabel(NewLabel),
    DeleteLabel(String),
}

pub struct KankanApp {
    runtime: Option<Runtime>,
    repository: Option<Repository>,
    workspace: Option<Workspace>,
    me: Option<User>,
    users: Vec<User>,
    projects: Vec<Project>,
    labels: Vec<crate::models::Label>,
    issues: Vec<IssueView>,
    detail: Option<IssueDetail>,
    selected_issue_id: Option<String>,
    selected_project_id: Option<String>,
    my_issues: bool,
    search: String,
    new_issue_open: bool,
    new_issue_form: NewIssueForm,
    edit_title: String,
    edit_description: String,
    edit_due_date: String,
    comment: String,
    subtask_title: String,
    manage_open: bool,
    manage_people_tab: bool,
    new_person_name: String,
    new_person_email: String,
    editing_person_id: Option<String>,
    edit_person_name: String,
    edit_person_email: String,
    pending_remove_person: Option<String>,
    new_category_name: String,
    new_category_color: usize,
    pending_remove_category: Option<String>,
    confirm_delete_issue: bool,
    error_message: Option<String>,
}

impl KankanApp {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&context.egui_ctx);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok();

        let mut app = Self {
            runtime,
            repository: None,
            workspace: None,
            me: None,
            users: Vec::new(),
            projects: Vec::new(),
            labels: Vec::new(),
            issues: Vec::new(),
            detail: None,
            selected_issue_id: None,
            selected_project_id: None,
            my_issues: false,
            search: String::new(),
            new_issue_open: false,
            new_issue_form: NewIssueForm::new("todo", &[], None),
            edit_title: String::new(),
            edit_description: String::new(),
            edit_due_date: String::new(),
            comment: String::new(),
            subtask_title: String::new(),
            manage_open: false,
            manage_people_tab: true,
            new_person_name: String::new(),
            new_person_email: String::new(),
            editing_person_id: None,
            edit_person_name: String::new(),
            edit_person_email: String::new(),
            pending_remove_person: None,
            new_category_name: String::new(),
            new_category_color: 0,
            pending_remove_category: None,
            confirm_delete_issue: false,
            error_message: None,
        };
        app.initialize();
        app
    }

    fn initialize(&mut self) {
        let Some(runtime) = &self.runtime else {
            self.error_message = Some("Could not start the async runtime.".into());
            return;
        };
        let setup = runtime.block_on(async {
            let path = db::database_path().map_err(crate::error::AppError::Other)?;
                        let pool = db::init_pool(&path).await?;
            db::seed_if_empty(&pool).await?;
            Ok::<_, crate::error::AppError>(Repository::new(pool))
        });
        match setup {
            Ok(repository) => {
                self.repository = Some(repository);
                if let Err(error) = self.load_initial_data() {
                    self.error_message = Some(error.to_string());
                }
            }
            Err(error) => self.error_message = Some(error.to_string()),
        }
    }

    fn load_initial_data(&mut self) -> AppResult<()> {
        let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) else {
            return Ok(());
        };
        let (workspace, me, users, projects, labels, issues) = runtime.block_on(async {
            Ok::<_, crate::error::AppError>((
                repository.workspace().await?,
                repository.current_user().await?,
                repository.users().await?,
                repository.projects().await?,
                repository.labels().await?,
                repository.issues(&IssueFilter::default()).await?,
            ))
        })?;
        self.workspace = Some(workspace);
        self.me = Some(me);
        self.users = users;
        self.projects = projects;
        self.labels = labels;
        self.issues = issues;
        self.new_issue_form = NewIssueForm::new("todo", &self.projects, self.me.as_ref());
        Ok(())
    }

    fn refresh_issues(&mut self) {
        if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
            match runtime.block_on(repository.issues(&IssueFilter::default())) {
                Ok(issues) => self.issues = issues,
                Err(error) => self.error_message = Some(error.to_string()),
            }
        }
    }

    fn refresh_workspace_catalog(&mut self) {
        let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) else {
            return;
        };
        let result = runtime.block_on(async {
            Ok::<_, crate::error::AppError>((
                repository.current_user().await?,
                repository.users().await?,
                repository.labels().await?,
            ))
        });
        match result {
            Ok((me, users, labels)) => {
                self.me = Some(me);
                self.users = users;
                self.labels = labels;
                self.refresh_issues();
                self.refresh_detail();
            }
            Err(error) => self.error_message = Some(error.to_string()),
        }
    }

    fn apply_workspace_action(&mut self, action: WorkspaceAction) {
        let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) else {
            return;
        };
        let result = match action {
            WorkspaceAction::CreateUser(data) => {
                runtime.block_on(repository.create_user(data)).map(|_| {
                    self.new_person_name.clear();
                    self.new_person_email.clear();
                })
            }
            WorkspaceAction::UpdateUser(id, data) => {
                runtime.block_on(repository.update_user(&id, data)).map(|_| {
                    self.editing_person_id = None;
                    self.edit_person_name.clear();
                    self.edit_person_email.clear();
                })
            }
            WorkspaceAction::SetCurrentUser(id) => {
                runtime.block_on(repository.set_current_user(&id)).map(|_| ())
            }
            WorkspaceAction::DeleteUser(id) => runtime.block_on(repository.delete_user(&id)),
            WorkspaceAction::CreateLabel(data) => {
                runtime.block_on(repository.create_label(data)).map(|_| {
                    self.new_category_name.clear();
                    self.pending_remove_category = None;
                })
            }
            WorkspaceAction::DeleteLabel(id) => runtime.block_on(repository.delete_label(&id)),
        };
        match result {
            Ok(()) => {
                self.error_message = None;
                self.pending_remove_person = None;
                self.pending_remove_category = None;
                self.refresh_workspace_catalog();
            }
            Err(error) => self.error_message = Some(error.to_string()),
        }
    }

    fn open_issue(&mut self, id: String) {
        let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) else {
            return;
        };
        match runtime.block_on(repository.issue(&id)) {
            Ok(detail) => {
                self.edit_title = detail.view.issue.title.clone();
                self.edit_description = detail.view.issue.description.clone();
                self.edit_due_date = detail
                    .view
                    .issue
                    .due_date
                    .clone()
                    .unwrap_or_default();
                self.detail = Some(detail);
                self.selected_issue_id = Some(id);
            }
            Err(error) => self.error_message = Some(error.to_string()),
        }
    }

    fn refresh_detail(&mut self) {
        if let (Some(runtime), Some(repository), Some(id)) =
            (&self.runtime, &self.repository, &self.selected_issue_id)
        {
            match runtime.block_on(repository.issue(id)) {
                Ok(detail) => {
                    self.edit_title = detail.view.issue.title.clone();
                    self.edit_description = detail.view.issue.description.clone();
                    self.edit_due_date = detail
                        .view
                        .issue
                        .due_date
                        .clone()
                        .unwrap_or_default();
                    self.detail = Some(detail);
                }
                Err(error) => self.error_message = Some(error.to_string()),
            }
        }
    }

    fn apply_actions(&mut self, actions: Vec<Action>) {
        for action in actions {
            let result = match action {
                Action::OpenIssue(id) => {
                    self.open_issue(id);
                    continue;
                }
                Action::CreateIssue(issue) => {
                    if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
                        runtime
                            .block_on(repository.create_issue(issue))
                            .map(|_| {
                                self.new_issue_open = false;
                                self.refresh_issues();
                            })
                    } else {
                        continue;
                    }
                }
                Action::UpdateIssue(id, update) => {
                    if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
                        runtime
                            .block_on(repository.update_issue(&id, update))
                            .map(|detail| {
                                self.refresh_issues();
                                if self.selected_issue_id.as_deref() == Some(id.as_str()) {
                                    self.edit_title = detail.view.issue.title.clone();
                                    self.edit_description = detail.view.issue.description.clone();
                                    self.edit_due_date = detail
                                        .view
                                        .issue
                                        .due_date
                                        .clone()
                                        .unwrap_or_default();
                                    self.detail = Some(detail);
                                }
                            })
                    } else {
                        continue;
                    }
                }
                Action::MoveIssue(id, status, sort_order) => {
                    if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
                        runtime
                            .block_on(repository.move_issue(&id, &status, sort_order))
                            .map(|_| self.refresh_issues())
                    } else {
                        continue;
                    }
                }
                Action::SetLabels(id, label_ids) => {
                    if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
                        runtime
                            .block_on(repository.set_labels(&id, &label_ids))
                            .map(|_| {
                                self.refresh_issues();
                                self.refresh_detail();
                            })
                    } else {
                        continue;
                    }
                }
                Action::DeleteIssue(id) => {
                    if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
                        runtime
                            .block_on(repository.delete_issue(&id))
                            .map(|_| {
                                self.confirm_delete_issue = false;
                                self.detail = None;
                                self.selected_issue_id = None;
                                self.refresh_issues();
                            })
                    } else {
                        continue;
                    }
                }
                Action::AddComment(id, body) => {
                    if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
                        runtime
                            .block_on(repository.add_comment(&id, &body))
                            .map(|_| self.refresh_detail())
                    } else {
                        continue;
                    }
                }
                Action::AddSubtask(id, title) => {
                    if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
                        runtime
                            .block_on(repository.add_subtask(&id, &title))
                            .map(|_| {
                                self.subtask_title.clear();
                                self.refresh_issues();
                                self.refresh_detail();
                            })
                    } else {
                        continue;
                    }
                }
                Action::ToggleSubtask(id) => {
                    if let (Some(runtime), Some(repository)) = (&self.runtime, &self.repository) {
                        runtime
                            .block_on(repository.toggle_subtask(&id))
                            .map(|_| {
                                self.refresh_issues();
                                self.refresh_detail();
                            })
                    } else {
                        continue;
                    }
                }
            };
            if let Err(error) = result {
                self.error_message = Some(error.to_string());
            } else {
                self.error_message = None;
            }
        }
    }

    fn filtered_issues(&self) -> Vec<IssueView> {
        let query = self.search.trim().to_lowercase();
        self.issues
            .iter()
            .filter(|issue| {
                self.selected_project_id
                    .as_deref()
                    .is_none_or(|project_id| issue.issue.project_id.as_deref() == Some(project_id))
            })
            .filter(|issue| {
                !self.my_issues
                    || self
                        .me
                        .as_ref()
                        .is_some_and(|me| issue.issue.assignee_id.as_deref() == Some(&me.id))
            })
            .filter(|issue| {
                query.is_empty()
                    || issue.issue.title.to_lowercase().contains(&query)
                    || issue.identifier.to_lowercase().contains(&query)
                    || issue.issue.description.to_lowercase().contains(&query)
                    || issue
                        .assignee
                        .as_ref()
                        .is_some_and(|assignee| assignee.name.to_lowercase().contains(&query))
                    || issue
                        .labels
                        .iter()
                        .any(|label| label.name.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }

    fn board(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let visible = self.filtered_issues();
        let open_count = visible
            .iter()
            .filter(|issue| !matches!(issue.issue.status.as_str(), "done" | "canceled"))
            .count();
        let done_count = visible
            .iter()
            .filter(|issue| issue.issue.status == "done")
            .count();
        let title = self
            .selected_project_id
            .as_ref()
            .and_then(|id| self.projects.iter().find(|project| &project.id == id))
            .map(|project| format!("{} {}", project.icon.as_deref().unwrap_or(""), project.name))
            .unwrap_or_else(|| {
                if self.my_issues {
                    "My Issues".to_owned()
                } else {
                    "All Issues".to_owned()
                }
            });

        ui.horizontal_wrapped(|ui| {
            let all_active = !self.my_issues && self.selected_project_id.is_none();
            if scope_button(ui, "All", all_active).clicked() {
                self.my_issues = false;
                self.selected_project_id = None;
            }
            if scope_button(ui, "Mine", self.my_issues).clicked() {
                self.my_issues = true;
                self.selected_project_id = None;
            }
            for project in self.projects.clone() {
                let active = self.selected_project_id.as_deref() == Some(&project.id);
                let label = format!(
                    "{} {}",
                    project.icon.as_deref().unwrap_or(""),
                    project.name
                )
                .trim()
                .to_owned();
                if scope_button(ui, &label, active).clicked() {
                    self.selected_project_id = Some(project.id);
                    self.my_issues = false;
                }
            }
            if scope_button(ui, "Manage", self.manage_open).clicked() {
                self.manage_open = true;
            }
        });
        ui.add_space(10.0);

        Frame::new()
            .fill(Color32::from_rgb(29, 30, 33))
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(CORNER_RADIUS)
            .inner_margin(Margin::symmetric(17, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&title).size(22.0).strong().color(TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, "+ New issue").clicked() {
                            self.new_issue_form =
                                NewIssueForm::new("todo", &self.projects, self.me.as_ref());
                            self.new_issue_open = true;
                        }
                        ui.add_sized(
                            [200.0, 34.0],
                            TextEdit::singleline(&mut self.search)
                                .id(egui::Id::new("board-search"))
                                .font(content_font(13.0))
                                .horizontal_align(Align::Center)
                                .vertical_align(Align::Center)
                                .hint_text("Search"),
                        );
                    });
                });
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    metric_chip(ui, "OPEN", open_count, ACCENT);
                    metric_chip(ui, "DONE", done_count, ACCENT);
                    for (status, name) in STATUSES {
                        let count = visible
                            .iter()
                            .filter(|issue| issue.issue.status == status)
                            .count();
                        flow_chip(ui, name, count, status_color(status));
                    }
                });
            });
        ui.add_space(16.0);

        let mut actions = Vec::new();
        ScrollArea::horizontal().id_salt("board-columns").show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for (status, name) in STATUSES {
                    let column_issues: Vec<_> = visible
                        .iter()
                        .filter(|issue| issue.issue.status == status)
                        .cloned()
                        .collect();
                    ui.allocate_ui_with_layout(
                        Vec2::new(COLUMN_WIDTH, ui.available_height()),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(COLUMN_WIDTH);
                            Frame::new()
                                .fill(status_tint(status))
                                .stroke(Stroke::new(1.0_f32, status_color(status)))
                                .corner_radius(CORNER_RADIUS)
                                .inner_margin(Margin::symmetric(8, 6))
                                .show(ui, |ui| {
                                    ui.set_width(COLUMN_WIDTH - 20.0);
                                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(status_glyph(status))
                                                    .color(status_color(status))
                                                    .size(15.0),
                                            );
                                            ui.label(RichText::new(name).strong().color(TEXT));
                                            ui.label(
                                                RichText::new(column_issues.len().to_string())
                                                    .color(MUTED),
                                            );
                                        });
                                    });
                                });
                            ui.add_space(8.0);
                            let (_, dropped) = ui.dnd_drop_zone::<String, _>(
                                Frame::new()
                                    .fill(Color32::from_rgb(24, 25, 28))
                                    .stroke(Stroke::new(1.0_f32, BORDER))
                                    .corner_radius(CORNER_RADIUS)
                                    .inner_margin(Margin::same(10)),
                                |ui| {
                                    ui.set_min_width(COLUMN_WIDTH - 20.0);
                                    ui.set_min_height(320.0);
                                    ScrollArea::vertical()
                                        .id_salt(status)
                                        .max_height(ui.available_height() - 10.0)
                                        .show(ui, |ui| {
                                            for issue in &column_issues {
                                                let response = issue_card(ui, issue);
                                                if response.clicked() {
                                                    actions.push(Action::OpenIssue(
                                                        issue.issue.id.clone(),
                                                    ));
                                                }
                                                ui.add_space(10.0);
                                            }
                                        });
                                },
                            );
                            if let Some(issue_id) = dropped {
                                let next_order = self
                                    .issues
                                    .iter()
                                    .filter(|candidate| candidate.issue.status == status)
                                    .map(|candidate| candidate.issue.sort_order)
                                    .fold(None, |highest, order| {
                                        Some(highest.map_or(order, |value: f64| value.max(order)))
                                    })
                                    .map_or(0.0, |highest| highest + 1.0);
                                actions.push(Action::MoveIssue(
                                    (*issue_id).clone(),
                                    status.to_owned(),
                                    next_order,
                                ));
                            }
                        },
                    );
                    ui.add_space(12.0);
                }
            });
        });
        self.apply_actions(actions);
        if ctx.input(|input| input.key_pressed(egui::Key::C) && !input.modifiers.command)
            && !ctx.wants_keyboard_input()
        {
            self.new_issue_form = NewIssueForm::new("todo", &self.projects, self.me.as_ref());
            self.new_issue_open = true;
        }
    }

    fn issue_detail(&mut self, ui: &mut egui::Ui) {
        let Some(detail) = self.detail.clone() else {
            ui.centered_and_justified(|ui| ui.label(RichText::new("Loading issue…").color(MUTED)));
            return;
        };

        let mut actions = Vec::new();
        ui.horizontal(|ui| {
            if subtle_button(ui, "‹ Board").clicked() {
                self.detail = None;
                self.selected_issue_id = None;
                self.confirm_delete_issue = false;
            }
            ui.label(RichText::new(" / ").color(FAINT));
            ui.label(RichText::new(&detail.view.identifier).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if danger_button(ui, "Delete issue").clicked() {
                    self.confirm_delete_issue = true;
                }
            });
        });
        if self.confirm_delete_issue {
            Frame::new()
                .fill(Color32::from_rgb(48, 31, 33))
                .corner_radius(CORNER_RADIUS)
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Delete this issue and its history permanently?")
                                .color(TEXT),
                        );
                        if danger_button(ui, "Delete permanently").clicked() {
                            actions.push(Action::DeleteIssue(detail.view.issue.id.clone()));
                        }
                        if subtle_button(ui, "Keep issue").clicked() {
                            self.confirm_delete_issue = false;
                        }
                    });
                });
            ui.add_space(8.0);
        }
        ui.add_space(16.0);
        ui.separator();

        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(18.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(470.0);
                    ui.horizontal(|ui| {
                        let title_width = (ui.available_width() - 100.0).max(260.0);
                        ui.add_sized(
                            [title_width, 42.0],
                            TextEdit::singleline(&mut self.edit_title)
                                .font(content_font(24.0))
                                .margin(Margin::symmetric(14, 8))
                                .hint_text("Issue title"),
                        );
                        if subtle_button(ui, "Save title").clicked()
                            && !self.edit_title.trim().is_empty()
                        {
                            actions.push(Action::UpdateIssue(
                                detail.view.issue.id.clone(),
                                UpdateIssue {
                                    title: Some(self.edit_title.trim().to_owned()),
                                    ..Default::default()
                                },
                            ));
                        }
                    });
                    ui.add_space(18.0);
                    ui.label(RichText::new("Description").color(MUTED).strong());
                    ui.add_space(5.0);
                    ui.add_sized(
                        [ui.available_width(), 125.0],
                        TextEdit::multiline(&mut self.edit_description)
                            .font(content_font(14.0))
                            .margin(Margin::symmetric(14, 10))
                            .hint_text("Add a description…")
                            .desired_rows(5),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if subtle_button(ui, "Save description").clicked() {
                            actions.push(Action::UpdateIssue(
                                detail.view.issue.id.clone(),
                                UpdateIssue {
                                    description: Some(self.edit_description.clone()),
                                    ..Default::default()
                                },
                            ));
                        }
                    });

                    ui.add_space(22.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Subtasks").size(15.0).strong().color(TEXT));
                        ui.label(
                            RichText::new(format!(
                                "{}/{}",
                                detail.view.subtask_done, detail.view.subtask_total
                            ))
                            .color(MUTED),
                        );
                    });
                    for subtask in &detail.subtasks {
                        let mut done = subtask.done == 1;
                        if ui
                            .checkbox(&mut done, &subtask.title)
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .changed()
                        {
                            actions.push(Action::ToggleSubtask(subtask.id.clone()));
                        }
                    }
                    ui.horizontal(|ui| {
                        let subtask_width = (ui.available_width() - 78.0).max(180.0);
                        ui.add_sized(
                            [subtask_width, 34.0],
                            TextEdit::singleline(&mut self.subtask_title)
                                .font(content_font(13.0))
                                .margin(Margin::symmetric(11, 6))
                                .hint_text("Add a subtask…"),
                        );
                        if subtle_button(ui, "Add").clicked()
                            && !self.subtask_title.trim().is_empty()
                        {
                            actions.push(Action::AddSubtask(
                                detail.view.issue.id.clone(),
                                self.subtask_title.trim().to_owned(),
                            ));
                        }
                    });

                    ui.add_space(22.0);
                    ui.label(RichText::new("Activity").size(15.0).strong().color(TEXT));
                    ui.add_space(8.0);
                    for activity in &detail.activities {
                        let actor = activity
                            .actor
                            .as_ref()
                            .map(|person| person.name.as_str())
                            .unwrap_or("Someone");
                        let line = if activity.activity.kind == "comment" {
                            format!(
                                "{actor}: {}",
                                activity.activity.body.as_deref().unwrap_or("")
                            )
                        } else {
                            format!(
                                "{actor} {}",
                                activity_description(&activity.activity.kind)
                            )
                        };
                        Frame::new()
                            .fill(PANEL)
                            .corner_radius(CORNER_RADIUS)
                            .inner_margin(9.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new(line).color(MUTED));
                            });
                        ui.add_space(5.0);
                    }
                    ui.add_space(8.0);
                    ui.add_sized(
                        [ui.available_width(), 72.0],
                        TextEdit::multiline(&mut self.comment)
                            .font(content_font(14.0))
                            .margin(Margin::symmetric(14, 10))
                            .hint_text("Leave a comment…"),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, "Comment").clicked()
                            && !self.comment.trim().is_empty()
                        {
                            actions.push(Action::AddComment(
                                detail.view.issue.id.clone(),
                                self.comment.trim().to_owned(),
                            ));
                            self.comment.clear();
                        }
                    });
                });
                ui.add_space(28.0);
                ui.separator();
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.set_min_width(235.0);
                    property_label(ui, "Status");
                    egui::ComboBox::from_id_salt("detail-status")
                        .selected_text(status_name(&detail.view.issue.status))
                        .width(190.0)
                        .show_ui(ui, |ui| {
                            for (status, name) in STATUSES {
                                if ui
                                    .selectable_label(detail.view.issue.status == status, name)
                                    .clicked()
                                {
                                    actions.push(Action::UpdateIssue(
                                        detail.view.issue.id.clone(),
                                        UpdateIssue {
                                            status: Some(status.to_owned()),
                                            ..Default::default()
                                        },
                                    ));
                                }
                            }
                        });

                    ui.add_space(14.0);
                    property_label(ui, "Assignee");
                    egui::ComboBox::from_id_salt("detail-assignee")
                        .selected_text(
                            detail
                                .view
                                .assignee
                                .as_ref()
                                .map(|user| user.name.as_str())
                                .unwrap_or("Unassigned"),
                        )
                        .width(190.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(detail.view.issue.assignee_id.is_none(), "Unassigned")
                                .clicked()
                            {
                                actions.push(Action::UpdateIssue(
                                    detail.view.issue.id.clone(),
                                    UpdateIssue {
                                        assignee_id: Some(None),
                                        ..Default::default()
                                    },
                                ));
                            }
                            for user in &self.users {
                                if ui
                                    .selectable_label(
                                        detail.view.issue.assignee_id.as_deref()
                                            == Some(user.id.as_str()),
                                        &user.name,
                                    )
                                    .clicked()
                                {
                                    actions.push(Action::UpdateIssue(
                                        detail.view.issue.id.clone(),
                                        UpdateIssue {
                                            assignee_id: Some(Some(user.id.clone())),
                                            ..Default::default()
                                        },
                                    ));
                                }
                            }
                        });

                    ui.add_space(14.0);
                    property_label(ui, "Priority");
                    egui::ComboBox::from_id_salt("detail-priority")
                        .selected_text(priority_name(detail.view.issue.priority))
                        .width(190.0)
                        .show_ui(ui, |ui| {
                            for (priority, name) in PRIORITIES {
                                if ui
                                    .selectable_label(detail.view.issue.priority == priority, name)
                                    .clicked()
                                {
                                    actions.push(Action::UpdateIssue(
                                        detail.view.issue.id.clone(),
                                        UpdateIssue {
                                            priority: Some(priority),
                                            ..Default::default()
                                        },
                                    ));
                                }
                            }
                        });

                    ui.add_space(14.0);
                    property_label(ui, "Project");
                    egui::ComboBox::from_id_salt("detail-project")
                        .selected_text(
                            detail
                                .project
                                .as_ref()
                                .map(|project| project.name.as_str())
                                .unwrap_or("No project"),
                        )
                        .width(190.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(detail.view.issue.project_id.is_none(), "No project")
                                .clicked()
                            {
                                actions.push(Action::UpdateIssue(
                                    detail.view.issue.id.clone(),
                                    UpdateIssue {
                                        project_id: Some(None),
                                        ..Default::default()
                                    },
                                ));
                            }
                            for project in &self.projects {
                                if ui
                                    .selectable_label(
                                        detail.view.issue.project_id.as_deref()
                                            == Some(project.id.as_str()),
                                        &project.name,
                                    )
                                    .clicked()
                                {
                                    actions.push(Action::UpdateIssue(
                                        detail.view.issue.id.clone(),
                                        UpdateIssue {
                                            project_id: Some(Some(project.id.clone())),
                                            ..Default::default()
                                        },
                                    ));
                                }
                            }
                        });

                    ui.add_space(14.0);
                    property_label(ui, "Due date");
                    ui.add_sized(
                        [190.0, 34.0],
                        TextEdit::singleline(&mut self.edit_due_date)
                            .font(content_font(13.0))
                            .margin(Margin::symmetric(10, 6))
                            .hint_text("YYYY-MM-DD"),
                    );
                    ui.horizontal(|ui| {
                        if subtle_button(ui, "Save date").clicked() {
                            let due_date = self.edit_due_date.trim();
                            actions.push(Action::UpdateIssue(
                                detail.view.issue.id.clone(),
                                UpdateIssue {
                                    due_date: Some(
                                        (!due_date.is_empty()).then(|| due_date.to_owned()),
                                    ),
                                    ..Default::default()
                                },
                            ));
                        }
                        if subtle_button(ui, "Clear").clicked() {
                            self.edit_due_date.clear();
                            actions.push(Action::UpdateIssue(
                                detail.view.issue.id.clone(),
                                UpdateIssue {
                                    due_date: Some(None),
                                    ..Default::default()
                                },
                            ));
                        }
                    });

                    ui.add_space(14.0);
                    property_label(ui, "Estimate");
                    egui::ComboBox::from_id_salt("detail-estimate")
                        .selected_text(
                            detail
                                .view
                                .issue
                                .estimate
                                .map(|estimate| format!("{estimate} points"))
                                .unwrap_or_else(|| "No estimate".to_owned()),
                        )
                        .width(190.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    detail.view.issue.estimate.is_none(),
                                    "No estimate",
                                )
                                .clicked()
                            {
                                actions.push(Action::UpdateIssue(
                                    detail.view.issue.id.clone(),
                                    UpdateIssue {
                                        estimate: Some(None),
                                        ..Default::default()
                                    },
                                ));
                            }
                            for estimate in [1, 2, 3, 5, 8, 13] {
                                if ui
                                    .selectable_label(
                                        detail.view.issue.estimate == Some(estimate),
                                        format!("{estimate} points"),
                                    )
                                    .clicked()
                                {
                                    actions.push(Action::UpdateIssue(
                                        detail.view.issue.id.clone(),
                                        UpdateIssue {
                                            estimate: Some(Some(estimate)),
                                            ..Default::default()
                                        },
                                    ));
                                }
                            }
                        });

                    ui.add_space(14.0);
                    property_label(ui, "Labels");
                    let current_labels: Vec<String> =
                        detail.view.labels.iter().map(|label| label.id.clone()).collect();
                    let mut next_labels = current_labels.clone();
                    for label in &self.labels {
                        let mut selected = current_labels.contains(&label.id);
                        if ui.checkbox(&mut selected, &label.name).changed() {
                            if selected {
                                next_labels.push(label.id.clone());
                            } else {
                                next_labels.retain(|id| id != &label.id);
                            }
                            actions.push(Action::SetLabels(
                                detail.view.issue.id.clone(),
                                next_labels.clone(),
                            ));
                        }
                    }
                });
            });
        });
        self.apply_actions(actions);
    }

    fn new_issue_window(&mut self, ctx: &Context) {
        if !self.new_issue_open {
            return;
        }
        let backdrop = ctx.animate_bool_with_time(
            egui::Id::new("new-issue-backdrop"),
            true,
            0.18,
        );
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("new-issue-backdrop"),
        ))
        .rect_filled(
            ctx.content_rect(),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, (backdrop * 138.0) as u8),
        );
        let mut open = self.new_issue_open;
        let mut requested_close = false;
        egui::Window::new("Create issue")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .fixed_size([590.0, 510.0])
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .order(egui::Order::Foreground)
            .frame(
                Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(39, 40, 44, 244))
                    .stroke(Stroke::new(1.0_f32, Color32::from_gray(91)))
                    .corner_radius(CORNER_RADIUS)
                    .inner_margin(Margin::same(22))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 12],
                        blur: 36,
                        spread: 2,
                        color: Color32::from_black_alpha(150),
                    }),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("New issue").size(21.0).strong().color(TEXT));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if subtle_button(ui, "×").clicked() {
                            requested_close = true;
                        }
                    });
                });
                ui.add_space(16.0);
                property_label(ui, "TITLE");
                ui.add_sized(
                    [ui.available_width(), 38.0],
                    TextEdit::singleline(&mut self.new_issue_form.title)
                        .font(content_font(18.0))
                        .margin(Margin::symmetric(14, 7)),
                );
                ui.add_space(13.0);
                property_label(ui, "DESCRIPTION");
                ui.add_sized(
                    [ui.available_width(), 112.0],
                    TextEdit::multiline(&mut self.new_issue_form.description)
                        .font(content_font(14.0))
                        .margin(Margin::symmetric(14, 10)),
                );
                ui.add_space(14.0);
                property_label(ui, "DETAILS");
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("new-status")
                        .selected_text(status_name(&self.new_issue_form.status))
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for (status, name) in STATUSES {
                                ui.selectable_value(
                                    &mut self.new_issue_form.status,
                                    status.to_owned(),
                                    name,
                                );
                            }
                        });
                    egui::ComboBox::from_id_salt("new-priority")
                        .selected_text(priority_name(self.new_issue_form.priority))
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for (priority, name) in PRIORITIES {
                                ui.selectable_value(
                                    &mut self.new_issue_form.priority,
                                    priority,
                                    name,
                                );
                            }
                        });
                    egui::ComboBox::from_id_salt("new-project")
                        .selected_text(
                            self.new_issue_form
                                .project_id
                                .as_ref()
                                .and_then(|id| self.projects.iter().find(|project| &project.id == id))
                                .map(|project| project.name.as_str())
                                .unwrap_or("No project"),
                        )
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.new_issue_form.project_id,
                                None,
                                "No project",
                            );
                            for project in &self.projects {
                                ui.selectable_value(
                                    &mut self.new_issue_form.project_id,
                                    Some(project.id.clone()),
                                    &project.name,
                                );
                            }
                        });
                    egui::ComboBox::from_id_salt("new-assignee")
                        .selected_text(
                            self.new_issue_form
                                .assignee_id
                                .as_ref()
                                .and_then(|id| self.users.iter().find(|user| &user.id == id))
                                .map(|user| user.name.as_str())
                                .unwrap_or("Unassigned"),
                        )
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.new_issue_form.assignee_id,
                                None,
                                "Unassigned",
                            );
                            for user in &self.users {
                                ui.selectable_value(
                                    &mut self.new_issue_form.assignee_id,
                                    Some(user.id.clone()),
                                    &user.name,
                                );
                            }
                        });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [220.0, 34.0],
                        TextEdit::singleline(&mut self.new_issue_form.due_date)
                            .font(content_font(13.0))
                            .margin(Margin::symmetric(10, 6))
                            .hint_text("Due date · YYYY-MM-DD"),
                    );
                    egui::ComboBox::from_id_salt("new-estimate")
                        .selected_text(
                            self.new_issue_form
                                .estimate
                                .map(|estimate| format!("{estimate} points"))
                                .unwrap_or_else(|| "No estimate".to_owned()),
                        )
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.new_issue_form.estimate,
                                None,
                                "No estimate",
                            );
                            for estimate in [1, 2, 3, 5, 8, 13] {
                                ui.selectable_value(
                                    &mut self.new_issue_form.estimate,
                                    Some(estimate),
                                    format!("{estimate} points"),
                                );
                            }
                        });
                });
                ui.add_space(12.0);
                property_label(ui, "LABELS");
                ui.add_space(3.0);
                ui.horizontal_wrapped(|ui| {
                    for label in &self.labels {
                        let mut selected = self.new_issue_form.label_ids.contains(&label.id);
                        let label_color = parse_color(&label.color);
                        let grow_shape = ui.painter().add(egui::Shape::Noop);
                        let response = ui.selectable_label(
                            selected,
                            RichText::new(&label.name).color(label_color),
                        );
                        animate_hover_growth(ui, &response, grow_shape, PANEL);
                        if response.clicked() {
                            selected = !selected;
                            if selected {
                                self.new_issue_form.label_ids.push(label.id.clone());
                            } else {
                                self.new_issue_form.label_ids.retain(|id| id != &label.id);
                            }
                        }
                    }
                });
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, "Create issue").clicked()
                        && !self.new_issue_form.title.trim().is_empty()
                    {
                        self.error_message = None;
                        let issue = self.new_issue_form.clone().into_issue();
                        self.apply_actions(vec![Action::CreateIssue(issue)]);
                        if self.error_message.is_none() {
                            self.new_issue_form =
                                NewIssueForm::new("todo", &self.projects, self.me.as_ref());
                            requested_close = true;
                        }
                    }
                    if subtle_button(ui, "Cancel").clicked() {
                        requested_close = true;
                    }
                });
            });
        self.new_issue_open = open && !requested_close;
    }

    fn workspace_window(&mut self, ctx: &Context) {
        if !self.manage_open {
            return;
        }
        let mut open = self.manage_open;
        let people = self.users.clone();
        let categories = self.labels.clone();
        let mut action = None;
        egui::Window::new("Workspace manager")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([640.0, 560.0])
            .min_size([560.0, 420.0])
            .order(egui::Order::Foreground)
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .corner_radius(CORNER_RADIUS)
                    .inner_margin(Margin::same(20)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Manage local people and issue categories.")
                        .color(MUTED),
                );
                ui.label(
                    RichText::new("Profiles are stored on this device; they are not shared or synced.")
                        .size(11.0)
                        .color(FAINT),
                );
                if let Some(message) = &self.error_message {
                    ui.add_space(6.0);
                    ui.colored_label(MUTED, message);
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.manage_people_tab, "People")
                        .clicked()
                    {
                        self.manage_people_tab = true;
                    }
                    if ui
                        .selectable_label(!self.manage_people_tab, "Categories")
                        .clicked()
                    {
                        self.manage_people_tab = false;
                    }
                });
                ui.separator();

                if self.manage_people_tab {
                    ui.label(RichText::new("Add a person").strong().color(TEXT));
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [175.0, 34.0],
                            TextEdit::singleline(&mut self.new_person_name)
                                .margin(Margin::symmetric(10, 6))
                                .hint_text("Name"),
                        );
                        ui.add_sized(
                            [220.0, 34.0],
                            TextEdit::singleline(&mut self.new_person_email)
                                .margin(Margin::symmetric(10, 6))
                                .hint_text("Email (optional)"),
                        );
                        let ready = !self.new_person_name.trim().is_empty();
                        if ui
                            .add_enabled(ready, egui::Button::new("Add person"))
                            .clicked()
                        {
                            action = Some(WorkspaceAction::CreateUser(NewUser {
                                name: self.new_person_name.clone(),
                                email: Some(self.new_person_email.clone()),
                            }));
                        }
                    });
                    ui.add_space(12.0);
                    ui.label(RichText::new("Profiles").strong().color(TEXT));
                    ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                        for person in &people {
                            Frame::new()
                                .fill(Color32::from_rgb(34, 35, 39))
                                .corner_radius(CORNER_RADIUS)
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    let editing =
                                        self.editing_person_id.as_deref() == Some(&person.id);
                                    if editing {
                                        ui.horizontal(|ui| {
                                            ui.add_sized(
                                                [160.0, 32.0],
                                                TextEdit::singleline(&mut self.edit_person_name)
                                                    .margin(Margin::symmetric(9, 5)),
                                            );
                                            ui.add_sized(
                                                [195.0, 32.0],
                                                TextEdit::singleline(&mut self.edit_person_email)
                                                    .margin(Margin::symmetric(9, 5))
                                                    .hint_text("Email (optional)"),
                                            );
                                            if subtle_button(ui, "Save").clicked() {
                                                action = Some(WorkspaceAction::UpdateUser(
                                                    person.id.clone(),
                                                    NewUser {
                                                        name: self.edit_person_name.clone(),
                                                        email: Some(
                                                            self.edit_person_email.clone(),
                                                        ),
                                                    },
                                                ));
                                            }
                                            if subtle_button(ui, "Cancel").clicked() {
                                                self.editing_person_id = None;
                                            }
                                        });
                                    } else {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(&person.name)
                                                        .strong()
                                                        .color(TEXT),
                                                );
                                                ui.label(
                                                    RichText::new(
                                                        person.email.as_deref().unwrap_or("No email"),
                                                    )
                                                    .size(11.0)
                                                    .color(FAINT),
                                                );
                                            });
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    if danger_button(ui, "Remove").clicked() {
                                                        self.pending_remove_person =
                                                            Some(person.id.clone());
                                                    }
                                                    if subtle_button(ui, "Edit").clicked() {
                                                        self.editing_person_id =
                                                            Some(person.id.clone());
                                                        self.edit_person_name =
                                                            person.name.clone();
                                                        self.edit_person_email = person
                                                            .email
                                                            .clone()
                                                            .unwrap_or_default();
                                                    }
                                                    if person.is_me == 1 {
                                                        ui.label(
                                                            RichText::new("Current profile")
                                                                .size(11.0)
                                                                .color(ACCENT),
                                                        );
                                                    } else if subtle_button(ui, "Use profile")
                                                        .clicked()
                                                    {
                                                        action = Some(
                                                            WorkspaceAction::SetCurrentUser(
                                                                person.id.clone(),
                                                            ),
                                                        );
                                                    }
                                                },
                                            );
                                        });
                                    }
                                    if self.pending_remove_person.as_deref()
                                        == Some(&person.id)
                                    {
                                        ui.add_space(6.0);
                                        ui.colored_label(
                                            MUTED,
                                            "Removing this person unassigns their tasks and keeps the profile history anonymous.",
                                        );
                                        ui.horizontal(|ui| {
                                            if danger_button(ui, "Remove person").clicked() {
                                                action = Some(WorkspaceAction::DeleteUser(
                                                    person.id.clone(),
                                                ));
                                            }
                                            if subtle_button(ui, "Keep").clicked() {
                                                self.pending_remove_person = None;
                                            }
                                        });
                                    }
                                });
                            ui.add_space(7.0);
                        }
                    });
                } else {
                    ui.label(RichText::new("Add a category").strong().color(TEXT));
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [210.0, 34.0],
                            TextEdit::singleline(&mut self.new_category_name)
                                .margin(Margin::symmetric(10, 6))
                                .hint_text("Category name"),
                        );
                        for (index, color) in CATEGORY_COLORS.iter().enumerate() {
                            let selected = self.new_category_color == index;
                            if ui
                                .selectable_label(
                                    selected,
                                    RichText::new("●").size(18.0).color(parse_color(color)),
                                )
                                .clicked()
                            {
                                self.new_category_color = index;
                            }
                        }
                        if ui
                            .add_enabled(
                                !self.new_category_name.trim().is_empty(),
                                egui::Button::new("Add"),
                            )
                            .clicked()
                        {
                            action = Some(WorkspaceAction::CreateLabel(NewLabel {
                                name: self.new_category_name.clone(),
                                color: CATEGORY_COLORS[self
                                    .new_category_color
                                    .min(CATEGORY_COLORS.len() - 1)]
                                .to_owned(),
                            }));
                        }
                    });
                    ui.add_space(12.0);
                    ui.label(RichText::new("Categories").strong().color(TEXT));
                    ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                        for category in &categories {
                            let issue_count = self
                                .issues
                                .iter()
                                .filter(|issue| {
                                    issue.labels.iter().any(|label| label.id == category.id)
                                })
                                .count();
                            Frame::new()
                                .fill(Color32::from_rgb(34, 35, 39))
                                .corner_radius(CORNER_RADIUS)
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("●").color(parse_color(&category.color)),
                                        );
                                        ui.label(
                                            RichText::new(&category.name)
                                                .strong()
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "{issue_count} {}",
                                                if issue_count == 1 { "issue" } else { "issues" }
                                            ))
                                            .size(11.0)
                                            .color(FAINT),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if danger_button(ui, "Remove").clicked() {
                                                    self.pending_remove_category =
                                                        Some(category.id.clone());
                                                }
                                            },
                                        );
                                    });
                                    if self.pending_remove_category.as_deref()
                                        == Some(&category.id)
                                    {
                                        ui.add_space(6.0);
                                        ui.colored_label(
                                            MUTED,
                                            format!(
                                                "This will remove the category from {issue_count} {}.",
                                                if issue_count == 1 { "issue" } else { "issues" }
                                            ),
                                        );
                                        ui.horizontal(|ui| {
                                            if danger_button(ui, "Remove category").clicked() {
                                                action = Some(WorkspaceAction::DeleteLabel(
                                                    category.id.clone(),
                                                ));
                                            }
                                            if subtle_button(ui, "Keep").clicked() {
                                                self.pending_remove_category = None;
                                            }
                                        });
                                    }
                                });
                            ui.add_space(7.0);
                        }
                    });
                }
            });
        self.manage_open = open;
        if let Some(action) = action {
            self.apply_workspace_action(action);
        }
    }
}

impl eframe::App for KankanApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1));
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::K)) {
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("board-search")));
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.new_issue_open {
                self.new_issue_open = false;
            } else if self.manage_open {
                self.manage_open = false;
            } else if self.confirm_delete_issue {
                self.confirm_delete_issue = false;
            } else if self.selected_issue_id.is_some() {
                self.detail = None;
                self.selected_issue_id = None;
            }
        }
        window_drag_region(ctx);
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(22, 16)))
            .show(ctx, |ui| {
                if let Some(message) = self.error_message.clone() {
                    ui.horizontal(|ui| {
                        ui.colored_label(MUTED, message);
                        if subtle_button(ui, "Dismiss").clicked() {
                            self.error_message = None;
                        }
                    });
                    ui.add_space(8.0);
                }
                if self.repository.is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("Kankan could not open its local database.")
                                .size(17.0)
                                .color(TEXT),
                        );
                    });
                } else if self.selected_issue_id.is_some() {
                    self.issue_detail(ui);
                } else {
                    self.board(ui, ctx);
                }
            });
        self.new_issue_window(ctx);
        self.workspace_window(ctx);
    }
}

fn apply_theme(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        UI_FONT.to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/Clarity_City/ClarityCity-VariableFont_wght.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        CONTENT_FONT.to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/Finlandica_Text/FinlandicaText-VariableFont_wght.ttf"
        ))
        .into(),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, UI_FONT.to_owned());
    fonts
        .families
        .insert(FontFamily::Name(CONTENT_FONT.into()), vec![CONTENT_FONT.to_owned()]);
    ctx.set_fonts(fonts);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(20, 21, 23);
    visuals.faint_bg_color = Color32::from_rgb(36, 37, 40);
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.noninteractive.corner_radius =
        egui::CornerRadius::same(CORNER_RADIUS as u8);
    visuals.widgets.inactive.bg_fill = PANEL;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, MUTED);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(CORNER_RADIUS as u8);
    visuals.widgets.hovered.bg_fill = PANEL_HOVER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(CORNER_RADIUS as u8);
    visuals.widgets.active.bg_fill = ACCENT_SOFT;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(CORNER_RADIUS as u8);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(CORNER_RADIUS as u8);
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(9.0, 9.0);
    style.spacing.button_padding = Vec2::new(10.0, 7.0);
    style.animation_time = 0.18;
    style.visuals.window_corner_radius = egui::CornerRadius::same(CORNER_RADIUS as u8);
    ctx.set_style(style);
}

fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let grow_shape = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(
        egui::Button::new(RichText::new(label).strong().color(Color32::from_gray(28)))
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .corner_radius(CORNER_RADIUS),
    );
    animate_hover_growth(ui, &response, grow_shape, ACCENT);
    response
}

fn subtle_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let fill = Color32::from_rgb(36, 37, 40);
    let grow_shape = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(
        egui::Button::new(RichText::new(label).color(MUTED))
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(CORNER_RADIUS),
    );
    animate_hover_growth(ui, &response, grow_shape, fill);
    response
}

fn danger_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let fill = Color32::from_rgb(36, 37, 40);
    let grow_shape = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(
        egui::Button::new(RichText::new(label).color(MUTED))
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(CORNER_RADIUS),
    );
    animate_hover_growth(ui, &response, grow_shape, fill);
    response
}

fn window_drag_region(ctx: &Context) {
    egui::TopBottomPanel::top("window_drag_region")
        .exact_height(30.0)
        .show_separator_line(false)
        .frame(Frame::new().fill(BG))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(82.0);
                let size = Vec2::new(ui.available_width(), ui.available_height());
                let (_, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                if response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            });
        });
}

fn scope_button(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let fill = if selected {
        Color32::from_rgb(50, 51, 55)
    } else {
        Color32::from_rgb(27, 28, 31)
    };
    let grow_shape = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(11.0)
                .strong()
                .color(if selected { TEXT } else { MUTED }),
        )
        .fill(fill)
        .stroke(Stroke::new(
            1.0_f32,
            if selected { Color32::from_gray(96) } else { BORDER },
        ))
        .corner_radius(CORNER_RADIUS),
    );
    animate_hover_growth(ui, &response, grow_shape, fill);
    response
}

fn animate_hover_growth(
    ui: &mut egui::Ui,
    response: &egui::Response,
    shape: egui::layers::ShapeIdx,
    fill: Color32,
) {
    let amount = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.16);
    if amount <= 0.01 {
        return;
    }
    ui.painter().set(
        shape,
        egui::epaint::RectShape::filled(
            response.rect.expand(2.0 * amount),
            CORNER_RADIUS,
            fill,
        ),
    );
}

fn metric_chip(ui: &mut egui::Ui, label: &str, value: usize, color: Color32) {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 27))
        .stroke(Stroke::new(1.0_f32, color))
        .corner_radius(CORNER_RADIUS)
        .inner_margin(Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(value.to_string()).strong().color(TEXT));
                ui.label(RichText::new(label).size(9.0).strong().color(color));
            });
        });
}

fn flow_chip(ui: &mut egui::Ui, label: &str, value: usize, color: Color32) {
    ui.label(
        RichText::new(format!("{}  {}", label.to_uppercase(), value))
            .size(10.0)
            .strong()
            .color(color),
    );
}

fn content_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(CONTENT_FONT.into()))
}

fn issue_card(ui: &mut egui::Ui, issue: &IssueView) -> egui::Response {
    let frame = Frame::new()
        .fill(Color32::from_rgb(35, 36, 40))
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CORNER_RADIUS)
        .inner_margin(Margin::same(12));
    let grow_shape = ui.painter().add(egui::Shape::Noop);
    let card = frame.show(ui, |ui| {
        ui.set_width(COLUMN_WIDTH - 44.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(priority_glyph(issue.issue.priority))
                    .color(priority_color(issue.issue.priority))
                    .size(12.0),
            );
            ui.label(
                RichText::new(&issue.identifier)
                    .color(MUTED)
                    .font(content_font(11.0))
                    .size(11.0),
            );
        });
        ui.add_space(5.0);
        ui.label(
            RichText::new(&issue.issue.title)
                .color(TEXT)
                .font(content_font(15.0))
                .strong(),
        );
        ui.add_space(7.0);
        ui.horizontal_wrapped(|ui| {
            for label in &issue.labels {
                let color = parse_color(&label.color);
                Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(
                        color.r(),
                        color.g(),
                        color.b(),
                        35,
                    ))
                    .corner_radius(6.0)
                    .inner_margin(Margin::symmetric(5, 2))
                    .show(ui, |ui| {
                        ui.label(RichText::new(&label.name).size(10.0).color(color));
                    });
            }
        });
        ui.horizontal(|ui| {
            if let Some(due_date) = &issue.issue.due_date {
                let overdue = chrono::NaiveDate::parse_from_str(due_date, "%Y-%m-%d")
                    .is_ok_and(|date| date < chrono::Utc::now().date_naive())
                    && issue.issue.status != "done"
                    && issue.issue.status != "canceled";
                let color = if overdue {
                    Color32::from_rgb(229, 93, 98)
                } else {
                    MUTED
                };
                ui.label(
                    RichText::new(format!("Due {due_date}"))
                        .font(content_font(10.0))
                        .color(color),
                );
            }
            if let Some(estimate) = issue.issue.estimate {
                ui.label(
                    RichText::new(format!("{estimate} pt"))
                        .font(content_font(10.0))
                        .color(MUTED),
                );
            }
            if let Some(branch) = &issue.issue.branch_ref {
                ui.label(RichText::new(format!("⌘ {branch}")).size(10.0).color(MUTED));
            }
            if issue.subtask_total > 0 {
                ui.label(
                    RichText::new(format!(
                        "☑ {}/{}",
                        issue.subtask_done, issue.subtask_total
                    ))
                    .size(10.0)
                    .color(MUTED),
                );
            }
            if let Some(assignee) = &issue.assignee {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(initials(&assignee.name))
                            .font(content_font(10.0))
                            .strong()
                            .color(TEXT),
                    );
                });
            }
        });
    });
    let response = ui
        .interact(
            card.response.rect,
            ui.id().with(("issue-card", &issue.issue.id)),
            egui::Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::Grab);
    response.dnd_set_drag_payload(issue.issue.id.clone());
    animate_hover_growth(
        ui,
        &response,
        grow_shape,
        Color32::from_rgb(35, 36, 40),
    );
    response
}

fn property_label(ui: &mut egui::Ui, label: &str) {
    ui.label(RichText::new(label).size(11.0).strong().color(FAINT));
}

fn status_name(status: &str) -> &'static str {
    STATUSES
        .iter()
        .find(|(id, _)| *id == status)
        .map(|(_, name)| *name)
        .unwrap_or("Unknown")
}

fn status_glyph(status: &str) -> &'static str {
    match status {
        "backlog" => "◌",
        "todo" => "○",
        "in_progress" => "◐",
        "done" => "✓",
        "canceled" => "⊘",
        _ => "•",
    }
}

fn status_color(status: &str) -> Color32 {
    match status {
        "backlog" => Color32::from_rgb(135, 141, 153),
        "todo" => Color32::from_rgb(170, 177, 188),
        "in_progress" => ACCENT,
        "done" => Color32::from_rgb(75, 183, 125),
        "canceled" => Color32::from_rgb(205, 100, 105),
        _ => MUTED,
    }
}

fn status_tint(status: &str) -> Color32 {
    let color = status_color(status);
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30)
}

fn priority_name(priority: i64) -> &'static str {
    PRIORITIES
        .iter()
        .find(|(id, _)| *id == priority)
        .map(|(_, name)| *name)
        .unwrap_or("No priority")
}

fn priority_glyph(priority: i64) -> &'static str {
    match priority {
        1 => "↓",
        2 => "≡",
        3 => "↑",
        4 => "‼",
        _ => "·",
    }
}

fn priority_color(priority: i64) -> Color32 {
    match priority {
        1 => Color32::from_gray(145),
        2 => Color32::from_gray(166),
        3 => Color32::from_gray(190),
        4 => Color32::from_gray(220),
        _ => FAINT,
    }
}

fn activity_description(kind: &str) -> &'static str {
    match kind {
        "created" => "created the issue",
        "status_changed" => "changed the status",
        "assigned" => "changed the assignee",
        "priority_changed" => "changed priority",
        "renamed" => "renamed the issue",
        _ => "updated the issue",
    }
}

fn initials(name: &str) -> String {
    let mut parts = name.split_whitespace();
    let Some(first) = parts.next() else {
        return String::new();
    };
    let last = parts.last().unwrap_or(first);
    let mut result = first.chars().next().into_iter().collect::<String>();
    if last != first {
        result.extend(last.chars().next());
    }
    result.to_uppercase()
}

fn parse_color(hex: &str) -> Color32 {
    let value = hex.trim_start_matches('#');
    if value.len() == 6 {
        if let Ok(rgb) = u32::from_str_radix(value, 16) {
            return Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        }
    }
    MUTED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_and_priority_names_cover_known_values() {
        assert_eq!(status_name("in_progress"), "In Progress");
        assert_eq!(status_name("unknown"), "Unknown");
        assert_eq!(priority_name(4), "Urgent");
        assert_eq!(priority_name(99), "No priority");
    }

    #[test]
    fn parses_six_digit_hex_colors() {
        assert_eq!(parse_color("#e5484d"), Color32::from_rgb(229, 72, 77));
        assert_eq!(parse_color("invalid"), MUTED);
    }

    #[test]
    fn initials_use_first_and_last_name() {
        assert_eq!(initials("Karri"), "K");
        assert_eq!(initials("Jori van Devon"), "JD");
    }
}
