use super::*;

const FULL_ID: &str = "abc123def456789012345678901234567890123456789012345678901234abcd";
const SHORT_ID: &str = "abc123def456";

#[test]
fn cgroup_v1_full_id() {
    let id = parse_container_id_from_path(&format!("/docker/{FULL_ID}"));
    assert_eq!(id.unwrap(), FULL_ID);
}

#[test]
fn cgroup_v1_with_subpath() {
    let id = parse_container_id_from_path(&format!("/docker/{FULL_ID}/somesubdir"));
    assert_eq!(id.unwrap(), FULL_ID);
}

#[test]
fn cgroup_v2_scope() {
    let id = parse_container_id_from_path(&format!(
        "/system.slice/docker-{SHORT_ID}.scope"
    ));
    assert_eq!(id.unwrap(), SHORT_ID);
}

#[test]
fn non_docker_path_returns_none() {
    assert!(parse_container_id_from_path("/user.slice/user-1000.slice").is_none());
    assert!(parse_container_id_from_path("/").is_none());
    assert!(parse_container_id_from_path("").is_none());
}

#[test]
fn too_short_id_returns_none() {
    assert!(parse_container_id_from_path("/docker/abc123").is_none());
    assert!(parse_container_id_from_path("/system.slice/docker-abc123.scope").is_none());
}

#[test]
fn non_hex_id_returns_none() {
    assert!(parse_container_id_from_path("/docker/not-a-valid-container-id-xyz").is_none());
}

#[test]
fn extracts_from_cgroup_v1_content() {
    let content = format!("12:devices:/docker/{FULL_ID}\n11:memory:/docker/{FULL_ID}\n0::/\n");
    assert_eq!(extract_from_cgroup(&content).unwrap(), FULL_ID);
}

#[test]
fn extracts_from_cgroup_v2_content() {
    let content = format!("0::/system.slice/docker-{SHORT_ID}.scope\n");
    assert_eq!(extract_from_cgroup(&content).unwrap(), SHORT_ID);
}

#[test]
fn non_docker_cgroup_returns_none() {
    let content = "0::/user.slice/user-1000.slice/session-1.scope\n12:devices:/\n";
    assert!(extract_from_cgroup(content).is_none());
}

#[test]
fn empty_cgroup_returns_none() {
    assert!(extract_from_cgroup("").is_none());
}
