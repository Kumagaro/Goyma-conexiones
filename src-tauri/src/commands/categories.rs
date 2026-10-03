#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rusqlite::Connection;

    use crate::commands::auth::require_session;
    use crate::errors::AppError;
    use crate::models::category::CategoryListItem;
    use crate::test_utils::{create_test_session, create_test_user, insert_category_fixture, setup_test_db};

    fn create_category_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        name: &str,
        description: Option<&str>,
    ) -> Result<CategoryListItem, AppError> {
        crate::commands::categories::create_category_inner(connection, now, name, description)
    }

    fn search_categories_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        text: &str,
    ) -> Result<Vec<crate::models::category::CategoryMatch>, AppError> {
        crate::commands::categories::search_categories_inner(connection, now, text)
    }

    #[test]
    fn create_category_saves_clean_name_and_description_and_rejects_duplicates_ignoring_case_and_accents() {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catadmin",
            "administrador",
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let item = create_category_for_test(&mut connection, now, "  Camión  ", Some("  Café molido  ")).unwrap();
        assert_eq!(item.name, "Camión");
        assert_eq!(item.description, Some("Café molido".to_owned()));

        let duplicate: Result<CategoryListItem, AppError> =
            create_category_for_test(&mut connection, now, "camion", Some("otra"));
        assert_eq!(duplicate.unwrap_err().code, "duplicateField");

        let duplicate2: Result<CategoryListItem, AppError> =
            create_category_for_test(&mut connection, now, "CAMIÓN", None);
        assert_eq!(duplicate2.unwrap_err().code, "duplicateField");

        let blank = create_category_for_test(&mut connection, now, "", None);
        assert_eq!(blank.unwrap_err().code, "validationFailed");

        let too_long = "x".repeat(51);
        let long_name = create_category_for_test(&mut connection, now, &too_long, None);
        assert_eq!(long_name.unwrap_err().code, "validationFailed");

        let long_description = "x".repeat(201);
        let bad_desc = create_category_for_test(&mut connection, now, "Pan", Some(&long_description));
        assert_eq!(bad_desc.unwrap_err().code, "validationFailed");

        let row: (String, Option<String>, String) = connection
            .query_row(
                "SELECT name, description, nameNormalized FROM categories WHERE id = ?1",
                [item.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, ("Camión".to_owned(), Some("Café molido".to_owned()), "camion".to_owned()));
    }

    #[test]
    fn search_categories_matches_normalized_names_and_exact_match_flag() {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catsearch",
            "contador",
            "+5350000002",
            "12345678902",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        insert_category_fixture(&connection, "Café", None);
        insert_category_fixture(&connection, "Panadería", None);
        insert_category_fixture(&connection, "Ñame", None);

        let results = search_categories_for_test(&mut connection, now, "caf").unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "Café");
        assert!(results[0].is_exact_match);

        let all = search_categories_for_test(&mut connection, now, "").unwrap();
        assert_eq!(all.len(), 3);

        let with_n = search_categories_for_test(&mut connection, now, "n").unwrap();
        assert!(with_n.iter().any(|item| item.name == "Ñame"));
        assert!(!with_n.iter().any(|item| item.name == "Panadería" && item.is_exact_match));

        let exact = search_categories_for_test(&mut connection, now, "ñame").unwrap();
        assert!(exact.iter().any(|item| item.name == "Ñame" && item.is_exact_match));
    }

    #[test]
    fn category_commands_require_session_and_allow_both_roles() {
        let now = Utc::now();
        let mut no_session = setup_test_db();
        let err = crate::commands::categories::create_category_inner(&mut no_session, now, "Nuevo", None);
        assert_eq!(err.unwrap_err().code, "sessionExpired");

        let mut expired = setup_test_db();
        let user_id = create_test_user(
            &expired,
            "expiredcat",
            "contador",
            "+5350000003",
            "12345678903",
            "ValidPass1!",
        );
        create_test_session(
            &expired,
            user_id,
            false,
            now - chrono::Duration::hours(4),
            now - chrono::Duration::hours(4),
        );
        assert_eq!(
            crate::commands::categories::create_category_inner(&mut expired, now, "Otro", None)
                .unwrap_err()
                .code,
            "sessionExpired"
        );

        let now2 = Utc::now();
        for role in ["administrador", "contador"] {
            let mut connection = setup_test_db();
            let user_id = create_test_user(
                &connection,
                role,
                if role == "administrador" { "+5350000004" } else { "+5350000005" },
                if role == "administrador" { "12345678904" } else { "12345678905" },
                "ValidPass1!",
            );
            create_test_session(&connection, user_id, false, now2, now2);
            let create_result = crate::commands::categories::create_category_inner(&mut connection, now2, "Permiso", None);
            assert!(create_result.is_ok(), "rol {role} fue rechazado por autorización");
        }
    }
}
