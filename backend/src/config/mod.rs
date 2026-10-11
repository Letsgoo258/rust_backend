pub mod app;
pub mod dashboard;
pub mod navigation;

pub use app::AppConfig;
#[allow(unused_imports)]
pub use dashboard::{
    AcademicsDashboardProfile, DashboardConfig, DashboardModuleCard, DashboardQuickAction,
    DashboardWidget, RoleDashboardProfile,
};
#[allow(unused_imports)]
pub use navigation::{NavigationConfig, NavigationItem, NavigationSection, NavigationTree};
