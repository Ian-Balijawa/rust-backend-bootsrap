use utoipa::{
    Modify,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};

#[derive(utoipa::OpenApi)]
#[openapi(modifiers(&SecurityAddon), paths(
    crate::controller::user::login_handler,
    crate::controller::user::register_handler,
    crate::controller::user::refresh_handler,
    crate::controller::user::logout_handler,
    crate::controller::user::me_handler,
    crate::controller::user::forgot_password_handler,
    crate::controller::user::reset_password_handler,
    crate::controller::user::change_password_handler,
    crate::controller::user::replace_role_handler,
    crate::controller::task::list_tasks_handler,
    crate::controller::task::get_task_handler,
    crate::controller::task::create_task_handler,
    crate::controller::task::update_task_handler,
    crate::controller::task::delete_task_handler,
    crate::controller::note::list,
    crate::controller::note::get,
    crate::controller::note::create,
    crate::controller::note::update,
    crate::controller::note::delete,
    crate::controller::file::upload,
    crate::controller::file::metadata,
    crate::controller::file::download,
    crate::controller::file::delete,
    crate::controller::scheduler::list_tasks_handler,
    crate::controller::scheduler::get_task_handler,
    crate::controller::scheduler::list_task_executions_handler,
    crate::controller::scheduler::create_task_handler,
    crate::controller::scheduler::run_task_handler,
    crate::controller::scheduler::delete_task_handler,
    crate::controller::system_config::list_handler,
    crate::controller::system_config::get_handler,
    crate::controller::system_config::create_handler,
    crate::controller::system_config::update_handler,
    crate::controller::system_config::delete_handler,
    crate::controller::health::health_handler,
    crate::controller::health::ready_handler,
    crate::controller::health::live_handler,
    crate::controller::health::config_handler,
))]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi
            .components
            .get_or_insert_with(Default::default)
            .add_security_scheme(
                "bearerAuth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
    }
}
