use super::*;

#[tokio::test]
async fn project_aliases_resolve_to_one_physical_identity() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let first_path = create_project_directory(&test_infrastructure, "case-sensitive");
    let second_path = create_project_directory(&test_infrastructure, "different-project");
    let (first_project, second_project) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let first_project = transaction
                .create_project("Case", &first_path, Default::default())
                .await?;
            let second_project = transaction
                .create_project("case", &second_path, Default::default())
                .await?;
            Ok((first_project, second_project))
        })
        .await
        .unwrap();

    let relative_alias = relative_path_from_current_directory(&first_path);
    assert_eq!(
        test_infrastructure
            .infrastructure
            .get_project_by_path(&relative_alias)
            .await
            .unwrap()
            .unwrap()
            .id,
        first_project.id
    );
    assert_eq!(second_project.name, "case");

    let duplicate_name_path = create_project_directory(&test_infrastructure, "duplicate-name");
    let duplicate_name = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project("Case", &duplicate_name_path, Default::default())
                .await
        })
        .await;
    assert!(duplicate_name.is_err());

    let duplicate_identity = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project("Another", first_path.join("."), Default::default())
                .await
        })
        .await;
    assert!(duplicate_identity.is_err());

    #[cfg(unix)]
    {
        let symlink_alias = test_infrastructure
            .temporary_directory
            .path()
            .join("project-symlink");
        std::os::unix::fs::symlink(&first_path, &symlink_alias).unwrap();
        assert_eq!(
            test_infrastructure
                .infrastructure
                .get_project_by_path(&symlink_alias)
                .await
                .unwrap()
                .unwrap()
                .id,
            first_project.id
        );
    }
}

#[tokio::test]
async fn project_path_updates_reject_different_directories() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project_path = create_project_directory(&test_infrastructure, "stable");
    let different_path = create_project_directory(&test_infrastructure, "different");
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project("Stable project", &project_path, Default::default())
                .await
        })
        .await
        .unwrap();

    let update_error = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_project(ProjectUpdateOptions::new(project.id).with_path(different_path))
                .await
        })
        .await
        .unwrap_err();

    assert_eq!(
        update_error.kind(),
        InfrastructureErrorKind::InvalidProjectPath
    );
}

#[tokio::test]
async fn missing_project_directories_are_rejected() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let missing_path = test_infrastructure
        .temporary_directory
        .path()
        .join("missing-project");
    let error = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project("Missing project", &missing_path, Default::default())
                .await
        })
        .await
        .unwrap_err();

    assert_eq!(error.kind(), InfrastructureErrorKind::InvalidProjectPath);
}

#[cfg(unix)]
#[tokio::test]
async fn canonical_project_paths_must_be_utf8() {
    use std::os::unix::ffi::OsStringExt;

    let test_infrastructure = create_initialized_test_infrastructure().await;
    let non_utf8_path = test_infrastructure
        .temporary_directory
        .path()
        .join(std::ffi::OsString::from_vec(vec![b'p', 0xff]));
    std::fs::create_dir(&non_utf8_path).unwrap();
    let error = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project("Non UTF-8 project", &non_utf8_path, Default::default())
                .await
        })
        .await
        .unwrap_err();

    assert_eq!(error.kind(), InfrastructureErrorKind::InvalidProjectPath);
}

#[tokio::test]
async fn empty_and_missing_project_updates_preserve_state() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project_path = create_project_directory(&test_infrastructure, "unchanged");
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project("Stable project", &project_path, Default::default())
                .await
        })
        .await
        .unwrap();
    let unchanged_project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_project(ProjectUpdateOptions::new(project.id))
                .await
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged_project.updated_at, project.updated_at);
    assert!(
        test_infrastructure
            .infrastructure
            .execute_db_actions(async |transaction| {
                transaction
                    .update_project(ProjectUpdateOptions::new(ProjectId::new()))
                    .await
            })
            .await
            .unwrap()
            .is_none()
    );
}
