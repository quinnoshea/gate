// Permission
// e.g hellas.admin
// e.g hellas.view

pub enum Permission {
    Admin,
    View,
}

pub struct Grant {
    pub user_id: String,
    pub org_id: String,
    pub permission: PermissionType,
}
