#[derive(Debug)]
pub struct Query<'a> {
    pub link_type: &'a str,
    pub link_text: &'a str,
    pub status: Option<&'a str>,
    pub link_fors: &'a Option<Vec<String>>,
    pub explicit_for: bool,
}
