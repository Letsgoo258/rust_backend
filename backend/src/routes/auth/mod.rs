use crate::{
    modules::auth::handler::{
        complete_school_onboarding, confirm_password_reset, confirm_user_grant, create_school,
        delete_school, get_school_profile, get_school_role_capabilities_matrix,
        get_session_signature, get_super_admin_login_history, get_super_admin_stats,
        get_user_dashboard, get_user_grants, get_user_navigation, invite_admin, login, logout, me,
        request_delete_otp, request_password_reset, request_user_grant_otp, request_verify_otp,
        revoke_user_grant, search_users_for_delegation, setup_school_details, setup_system,
        submit_verify_otp, switch_tenant, update_onboarding_step, update_role_capabilities,
        update_school_status,
    },
    state::AppState,
};
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/add-admin", post(invite_admin))
        .route("/setup", post(setup_system))
        .route("/schools", post(create_school))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/session/sign", get(get_session_signature))
        .route("/verify/request-otp", post(request_verify_otp))
        .route("/verify/submit-otp", post(submit_verify_otp))
        .route("/password-reset/request", post(request_password_reset))
        .route("/password-reset/confirm", post(confirm_password_reset))
        .route("/schools/profile", get(get_school_profile))
        .route("/schools/setup", post(setup_school_details))
        .route("/schools/onboarding-step", patch(update_onboarding_step))
        .route(
            "/schools/complete-onboarding",
            post(complete_school_onboarding),
        )
        .route("/switch-tenant", post(switch_tenant))
        .route("/super-admin/stats", get(get_super_admin_stats))
        .route(
            "/super-admin/login-history",
            get(get_super_admin_login_history),
        )
        .route(
            "/super-admin/schools/:id/status",
            patch(update_school_status),
        )
        .route(
            "/super-admin/schools/:id/request-delete-otp",
            post(request_delete_otp),
        )
        .route("/super-admin/schools/:id/delete", post(delete_school))
        .route(
            "/permissions/matrix",
            get(get_school_role_capabilities_matrix),
        )
        .route(
            "/permissions/roles/:role_code",
            put(update_role_capabilities),
        )
        .route(
            "/permissions/users/search",
            get(search_users_for_delegation),
        )
        .route("/permissions/user-grants", get(get_user_grants))
        .route(
            "/permissions/user-grants/request-otp",
            post(request_user_grant_otp),
        )
        .route("/permissions/user-grants/confirm", post(confirm_user_grant))
        .route("/permissions/user-grants/:id", delete(revoke_user_grant))
        .route("/navigation", get(get_user_navigation))
        .route("/dashboard-config", get(get_user_dashboard))
}
