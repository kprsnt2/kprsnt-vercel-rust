use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use minijinja::{context, Environment};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    jinja: Arc<Environment<'static>>,
    portfolio_data: Arc<Value>,
    base_dir: PathBuf,
}

impl AppState {
    fn new() -> Self {
        let base_dir = std::env::var("LAMBDA_TASK_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let templates_dir = if base_dir.join("templates").exists() {
            base_dir.join("templates")
        } else if PathBuf::from("templates").exists() {
            PathBuf::from("templates")
        } else {
            base_dir.join("templates")
        };

        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader(&templates_dir));

        // Global `request` so templates checking `request.path` succeed
        env.add_global("request", minijinja::context! { path => "/" });

        // Global `url_for` for Jinja2/Flask template compatibility
        env.add_function(
            "url_for",
            |_endpoint: &str, kwargs: minijinja::value::Kwargs| -> Result<String, minijinja::Error> {
                let filename: Option<String> = kwargs.get("filename")?;
                if let Some(f) = filename {
                    Ok(format!("/static/{f}"))
                } else {
                    Ok("/static".to_string())
                }
            },
        );

        // Python/Jinja2 compatibility for `.get(key, default)` and `.startswith(prefix)`
        env.set_unknown_method_callback(|_state, value, method, args| {
            if method == "get" {
                if let Some(key) = args.first() {
                    if let Ok(item) = value.get_item(key) {
                        if !item.is_undefined() {
                            return Ok(item);
                        }
                    }
                    if let Some(default) = args.get(1) {
                        return Ok(default.clone());
                    }
                    return Ok(minijinja::Value::from(()));
                }
            }
            if method == "startswith" {
                if let Some(prefix) = args.first().and_then(|v| v.as_str()) {
                    if let Some(s) = value.as_str() {
                        return Ok(minijinja::Value::from(s.starts_with(prefix)));
                    }
                }
                return Ok(minijinja::Value::from(false));
            }
            Err(minijinja::Error::from(minijinja::ErrorKind::UnknownMethod))
        });

        let data_path = if base_dir.join("data").join("portfolio.json").exists() {
            base_dir.join("data").join("portfolio.json")
        } else {
            PathBuf::from("data").join("portfolio.json")
        };
        let portfolio_data: Value = if data_path.exists() {
            let s = fs::read_to_string(&data_path).unwrap_or_else(|_| "{}".to_string());
            serde_json::from_str(&s).unwrap_or(Value::Null)
        } else {
            Value::Null
        };

        Self {
            jinja: Arc::new(env),
            portfolio_data: Arc::new(portfolio_data),
            base_dir,
        }
    }

    fn render<S: Serialize>(&self, template_name: &str, ctx: S) -> Result<Html<String>, (StatusCode, String)> {
        let tmpl = self
            .jinja
            .get_template(template_name)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {e}")))?;

        let rendered = tmpl
            .render(ctx)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Render error: {e}")))?;

        Ok(Html(rendered))
    }

    fn load_json(&self, relative_path: &str) -> Value {
        let primary = self.base_dir.join(relative_path);
        let fallback = PathBuf::from(relative_path);
        let full_path = if primary.exists() { primary } else { fallback };
        if full_path.exists() {
            let content = fs::read_to_string(full_path).unwrap_or_else(|_| "{}".to_string());
            serde_json::from_str(&content).unwrap_or(Value::Null)
        } else {
            Value::Null
        }
    }
}

// -------------------------------------------------------------
// Page Handlers
// -------------------------------------------------------------

async fn about(State(state): State<AppState>) -> impl IntoResponse {
    state.render("about.html", context! {})
}

async fn skills(State(state): State<AppState>) -> impl IntoResponse {
    let skills = &state.portfolio_data["skills"];
    state.render("skills.html", context! { skills => skills })
}

async fn projects(State(state): State<AppState>) -> impl IntoResponse {
    let projects = &state.portfolio_data["projects"];
    state.render("projects.html", context! { projects => projects })
}

async fn resume(State(state): State<AppState>) -> impl IntoResponse {
    let resume = &state.portfolio_data["resume"];
    let projects = &state.portfolio_data["resume_projects"];
    let skills = &state.portfolio_data["resume_skills"];
    let roles = &state.portfolio_data["roles"];
    let education = &state.portfolio_data["education"];

    state.render(
        "resume.html",
        context! {
            projects => projects,
            skills => skills,
            role_definitions => roles,
            education => education,
            current_role => Value::Null,
            resume_summary => &resume["summary"]
        },
    )
}

async fn resume_role(
    Path(role_slug): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let resume = &state.portfolio_data["resume"];
    let projects = &state.portfolio_data["resume_projects"];
    let skills = &state.portfolio_data["resume_skills"];
    let roles = &state.portfolio_data["roles"];
    let education = &state.portfolio_data["education"];

    state.render(
        "resume.html",
        context! {
            projects => projects,
            skills => skills,
            role_definitions => roles,
            education => education,
            current_role => role_slug,
            resume_summary => &resume["summary"]
        },
    )
}

async fn plotter(State(state): State<AppState>) -> impl IntoResponse {
    state.render("plotter.html", context! {})
}

async fn docs(State(state): State<AppState>) -> impl IntoResponse {
    state.render("docs.html", context! {})
}

async fn aie(State(state): State<AppState>) -> impl IntoResponse {
    let telemetry = state.load_json("job_data/ecosystem_telemetry.json");
    state.render("aie.html", context! { telemetry => telemetry })
}

async fn blog(State(state): State<AppState>) -> impl IntoResponse {
    let mut posts = Vec::new();
    let blog_dir = if state.base_dir.join("blog_data").exists() {
        state.base_dir.join("blog_data")
    } else {
        PathBuf::from("blog_data")
    };
    if blog_dir.exists() {
        if let Ok(entries) = fs::read_dir(blog_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map_or(false, |ext| ext == "json") {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if let Ok(val) = serde_json::from_str::<Value>(&content) {
                            posts.push(val);
                        }
                    }
                }
            }
        }
    }
    state.render("blog.html", context! { posts => posts, categories => vec!["AI", "Data", "Engineering"] })
}

async fn blog_post(
    Path(slug): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let direct = state.base_dir.join("blog_data").join(format!("{slug}.json"));
    let file_path = if direct.exists() {
        direct
    } else {
        PathBuf::from("blog_data").join(format!("{slug}.json"))
    };
    if file_path.exists() {
        if let Ok(content) = fs::read_to_string(file_path) {
            if let Ok(post) = serde_json::from_str::<Value>(&content) {
                return state.render("blog_post.html", context! { post => post });
            }
        }
    }
    state.render("404.html", context! {})
}

async fn jobs(State(state): State<AppState>) -> impl IntoResponse {
    let telemetry = state.load_json("job_data/ecosystem_telemetry.json");
    state.render("jobs.html", context! { telemetry => telemetry })
}

async fn pharma(State(state): State<AppState>) -> impl IntoResponse {
    let pharma_log = state.load_json("job_data/pharma_pipeline_log.json");
    state.render("pharma.html", context! { log_data => pharma_log })
}

async fn brand(State(state): State<AppState>) -> impl IntoResponse {
    let brand_data = state.load_json("job_data/brand_timeseries.json");
    state.render("brand.html", context! { data => brand_data })
}

async fn ecosystem(State(state): State<AppState>) -> impl IntoResponse {
    let data = state.load_json("job_data/ecosystem_telemetry.json");
    state.render("ecosystem.html", context! { data => data })
}

async fn mcp_page(State(state): State<AppState>) -> impl IntoResponse {
    state.render("mcp.html", context! {})
}

// -------------------------------------------------------------
// Data APIs
// -------------------------------------------------------------

async fn api_jobs_data(State(state): State<AppState>) -> Json<Value> {
    Json(state.load_json("job_data/pipeline_log.json"))
}

async fn api_pharma_data(State(state): State<AppState>) -> Json<Value> {
    Json(state.load_json("job_data/pharma_pipeline_log.json"))
}

async fn api_brand_data(State(state): State<AppState>) -> Json<Value> {
    Json(state.load_json("job_data/brand_timeseries.json"))
}

async fn api_mcp_get() -> Json<Value> {
    Json(serde_json::json!({
        "status": "active",
        "server": "kprsnt-rust-mcp",
        "version": "1.0.0",
        "protocol": "MCP 2024-11-05",
        "runtime": "Rust / Axum (Zero Cold Start)",
        "tools": [
            "get_site_overview",
            "get_site_projects",
            "get_my_profile",
            "get_my_resume",
            "get_my_skills",
            "evaluate_job_match",
            "get_ai_eco_telemetry"
        ]
    }))
}

async fn api_mcp_post(Json(payload): Json<Value>) -> Json<Value> {
    let method = payload.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = payload.get("id").cloned().unwrap_or(Value::Null);

    let result = match method {
        "tools/list" => serde_json::json!({
            "tools": [
                {
                    "name": "get_site_overview",
                    "description": "Returns verified portfolio overview and real-time metrics"
                },
                {
                    "name": "get_my_skills",
                    "description": "Returns verified skills across AI, ML, Data, Cloud"
                }
            ]
        }),
        _ => serde_json::json!({ "status": "ok", "echo": method }),
    };

    Json(serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
}

// -------------------------------------------------------------
// Router Factory
// -------------------------------------------------------------

pub fn create_router() -> Router {
    let state = AppState::new();

    Router::new()
        .route("/", get(about))
        .route("/skills", get(skills))
        .route("/projects", get(projects))
        .route("/resume", get(resume))
        .route("/resume/:role_slug", get(resume_role))
        .route("/plotter", get(plotter))
        .route("/docs", get(docs))
        .route("/api/docs", get(docs))
        .route("/aie", get(aie))
        .route("/blog", get(blog))
        .route("/blog/:slug", get(blog_post))
        .route("/jobs", get(jobs))
        .route("/jobs/dashboard", get(jobs))
        .route("/pharma", get(pharma))
        .route("/brand", get(brand))
        .route("/ecosystem", get(ecosystem))
        .route("/mcp", get(mcp_page))
        .route("/api/jobs/data", get(api_jobs_data))
        .route("/api/pharma/data", get(api_pharma_data))
        .route("/api/brand/data", get(api_brand_data))
        .route("/api/mcp", get(api_mcp_get).post(api_mcp_post))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}

// -------------------------------------------------------------
// Entrypoint: Vercel Lambda / Local Dual Mode
// -------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    let app = create_router();

    // Run in serverless Lambda mode if on Vercel / AWS Lambda:
    let is_serverless = std::env::var("AWS_LAMBDA_RUNTIME_API").is_ok()
        || std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok()
        || std::env::var("LAMBDA_TASK_ROOT").is_ok()
        || std::env::var("VERCEL").is_ok();

    // Ensure lambda_runtime's Config::from_env() does not panic on missing variables in Vercel:
    if std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_err() {
        std::env::set_var("AWS_LAMBDA_FUNCTION_NAME", "kprsnt-main");
    }
    if std::env::var("AWS_LAMBDA_FUNCTION_MEMORY_SIZE").is_err() {
        std::env::set_var("AWS_LAMBDA_FUNCTION_MEMORY_SIZE", "1024");
    }
    if std::env::var("AWS_LAMBDA_FUNCTION_VERSION").is_err() {
        std::env::set_var("AWS_LAMBDA_FUNCTION_VERSION", "$LATEST");
    }

    if is_serverless {
        lambda_http::run(app).await
    } else {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
            .await
            .expect("Failed to bind 127.0.0.1:3000");
        println!("🚀 Rust Axum server running locally at http://127.0.0.1:3000");
        axum::serve(listener, app)
            .await
            .expect("Server error");
        Ok(())
    }
}
