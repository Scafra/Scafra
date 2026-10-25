use scafra_security::{BasicAuthConfig, JwtConfig, SecurityConfig};

const BASIC_PASSWORD_SECRET: &str = "basic-secret-sentinel";
const JWT_SECRET: &str = "jwt-secret-sentinel";

#[test]
fn defaults_and_partial_configuration_deserialize_consistently() {
    let config = SecurityConfig::default();
    assert!(!config.enabled);
    assert!(!config.hide_unauthorized);
    assert!(config.bearer_token.is_none());
    assert!(config.basic.username.is_none());
    assert!(config.basic.password.is_none());
    assert!(config.permit_all.is_empty());
    assert!(!config.jwt.enabled);
    assert_eq!(config.jwt.leeway_seconds, 60);
    assert_eq!(config.jwt.cache_ttl_seconds, 300);
    assert_eq!(config.jwt.refresh_cooldown_seconds, 30);
    assert_eq!(config.jwt.connect_timeout_ms, 2000);
    assert_eq!(config.jwt.request_timeout_ms, 5000);
    assert_eq!(config.validate(), Ok(()));

    let config: SecurityConfig =
        serde_json::from_str(r#"{"enabled":true,"bearer_token":"configured-token"}"#)
            .expect("the partial security configuration should deserialize");
    assert!(config.enabled);
    assert_eq!(config.bearer_token.as_deref(), Some("configured-token"));
    assert_eq!(config.validate(), Ok(()));
}

#[cfg(feature = "jwks")]
#[test]
fn remote_policy_bounds_are_validated_even_when_authentication_is_disabled() {
    let cases = [
        ("cache_ttl_seconds", 0),
        ("cache_ttl_seconds", 86401),
        ("refresh_cooldown_seconds", 0),
        ("refresh_cooldown_seconds", 3601),
        ("connect_timeout_ms", 0),
        ("connect_timeout_ms", 300001),
        ("request_timeout_ms", 0),
        ("request_timeout_ms", 300001),
    ];
    for (field, value) in cases {
        let mut input = serde_json::to_value(JwtConfig::default()).unwrap();
        input[field] = value.into();
        let config = SecurityConfig {
            jwt: serde_json::from_value(input).unwrap(),
            ..Default::default()
        };
        let error = config.validate().expect_err("invalid policy must fail");
        assert!(error.contains(field), "{field}: {error}");
    }
    for jwt in [
        JwtConfig {
            cache_ttl_seconds: 5,
            refresh_cooldown_seconds: 6,
            ..Default::default()
        },
        JwtConfig {
            connect_timeout_ms: 50,
            request_timeout_ms: 49,
            ..Default::default()
        },
    ] {
        assert!(SecurityConfig {
            jwt,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
    assert!(SecurityConfig {
        jwt: JwtConfig {
            cache_ttl_seconds: 86400,
            refresh_cooldown_seconds: 3600,
            connect_timeout_ms: 300000,
            request_timeout_ms: 300000,
            ..Default::default()
        },
        ..Default::default()
    }
    .validate()
    .is_ok());
}

#[cfg(not(feature = "jwks"))]
#[test]
fn unavailable_remote_policy_is_rejected_even_with_authentication_disabled() {
    for field in [
        "cache_ttl_seconds",
        "refresh_cooldown_seconds",
        "connect_timeout_ms",
        "request_timeout_ms",
    ] {
        let mut input = serde_json::to_value(JwtConfig::default()).unwrap();
        input[field] = 1.into();
        let config = SecurityConfig {
            jwt: serde_json::from_value(input).unwrap(),
            ..Default::default()
        };
        assert!(config.validate().is_err(), "{field}");
    }
}

#[test]
fn remote_policy_deserialization_rejects_negative_and_overflowing_values() {
    for field in [
        "leeway_seconds",
        "cache_ttl_seconds",
        "refresh_cooldown_seconds",
        "connect_timeout_ms",
        "request_timeout_ms",
    ] {
        for value in ["-1", "18446744073709551616"] {
            let input = format!("{{\"{field}\":{value}}}");
            assert!(serde_json::from_str::<JwtConfig>(&input).is_err());
        }
    }
}

#[cfg(feature = "jwt")]
#[test]
fn jwt_clock_skew_is_bounded_and_can_be_disabled_explicitly() {
    for leeway in [0, 300] {
        assert!(SecurityConfig {
            jwt: JwtConfig {
                leeway_seconds: leeway,
                ..Default::default()
            },
            ..Default::default()
        }
        .validate()
        .is_ok());
    }
    assert_eq!(
        SecurityConfig {
            jwt: JwtConfig {
                leeway_seconds: 301,
                ..Default::default()
            },
            ..Default::default()
        }
        .validate(),
        Err("JWT leeway_seconds must be between 0 and 300 seconds")
    );
}

#[test]
fn invalid_authentication_configurations_return_safe_validation_errors() {
    let cases = [
        (
            SecurityConfig {
                enabled: true,
                ..SecurityConfig::default()
            },
            "a bearer token or complete basic credentials are required",
        ),
        (
            SecurityConfig {
                enabled: true,
                bearer_token: Some(String::new()),
                ..SecurityConfig::default()
            },
            "the bearer token must not be empty or whitespace-only",
        ),
        (
            SecurityConfig {
                enabled: true,
                basic: BasicAuthConfig {
                    username: Some("user".to_owned()),
                    password: None,
                },
                ..SecurityConfig::default()
            },
            "a bearer token or complete basic credentials are required",
        ),
        (
            SecurityConfig {
                enabled: true,
                basic: BasicAuthConfig {
                    username: Some(String::new()),
                    password: Some("password".to_owned()),
                },
                ..SecurityConfig::default()
            },
            "basic credentials must not be empty",
        ),
        (
            SecurityConfig {
                enabled: true,
                basic: BasicAuthConfig {
                    username: Some("user".to_owned()),
                    password: Some(String::new()),
                },
                ..SecurityConfig::default()
            },
            "basic credentials must not be empty",
        ),
        #[cfg(feature = "jwt")]
        (
            SecurityConfig {
                jwt: JwtConfig {
                    enabled: true,
                    ..JwtConfig::default()
                },
                ..SecurityConfig::default()
            },
            "JWT requires secret, issuer_uri, or jwk_set_uri",
        ),
        #[cfg(feature = "jwt")]
        (
            SecurityConfig {
                jwt: JwtConfig {
                    enabled: true,
                    secret: Some(String::new()),
                    ..JwtConfig::default()
                },
                ..SecurityConfig::default()
            },
            "the JWT secret must not be empty",
        ),
    ];

    for (config, expected) in cases {
        assert_eq!(config.validate(), Err(expected));
    }
}

#[test]
#[cfg(feature = "jwks")]
fn jwt_provider_configuration_is_validated_without_echoing_urls() {
    let cases = [
        (
            JwtConfig {
                enabled: true,
                issuer_uri: Some("   ".to_owned()),
                ..JwtConfig::default()
            },
            "JWT issuer_uri must be a valid HTTP(S) URL",
        ),
        (
            JwtConfig {
                enabled: true,
                jwk_set_uri: Some("file:///tmp/provider-uri-sentinel".to_owned()),
                ..JwtConfig::default()
            },
            "JWT jwk_set_uri must be a valid HTTP(S) URL",
        ),
        (
            JwtConfig {
                enabled: true,
                secret: Some("hmac-secret".to_owned()),
                jwk_set_uri: Some("https://keys.test/keys".to_owned()),
                ..JwtConfig::default()
            },
            "JWT secret cannot be combined with remote JWKS configuration",
        ),
        (
            JwtConfig {
                enabled: true,
                secret: Some("hmac-secret".to_owned()),
                audiences: vec![" ".to_owned()],
                ..JwtConfig::default()
            },
            "JWT audiences must not contain empty or whitespace-only values",
        ),
    ];

    for (jwt, expected) in cases {
        let config = SecurityConfig {
            jwt,
            ..SecurityConfig::default()
        };
        let error = config.validate().expect_err("invalid JWT config must fail");
        assert_eq!(error, expected);
        assert!(!format!("{error} {config:?}").contains("provider-uri-sentinel"));
    }

    let legacy_hmac_with_issuer = SecurityConfig {
        jwt: JwtConfig {
            enabled: true,
            secret: Some("hmac-secret".to_owned()),
            issuer_uri: Some("https://issuer.test".to_owned()),
            ..JwtConfig::default()
        },
        ..SecurityConfig::default()
    };
    assert_eq!(legacy_hmac_with_issuer.validate(), Ok(()));
}

#[test]
fn application_bearer_validation_rejects_blank_values_even_with_alternatives() {
    for token in ["", " ", "   ", "\t", " \t "] {
        let config = SecurityConfig {
            enabled: true,
            bearer_token: Some(token.to_owned()),
            ..SecurityConfig::default()
        };
        assert_eq!(
            config.validate(),
            Err("the bearer token must not be empty or whitespace-only"),
            "blank token {token:?} must fail validation"
        );

        let config_with_basic = SecurityConfig {
            enabled: true,
            bearer_token: Some(token.to_owned()),
            basic: BasicAuthConfig {
                username: Some("user".to_owned()),
                password: Some("password".to_owned()),
            },
            ..SecurityConfig::default()
        };
        assert_eq!(
            config_with_basic.validate(),
            Err("the bearer token must not be empty or whitespace-only"),
            "a complete Basic authenticator must not preserve blank token {token:?}"
        );

        let config_with_jwt = SecurityConfig {
            enabled: true,
            bearer_token: Some(token.to_owned()),
            jwt: JwtConfig {
                enabled: true,
                secret: Some("jwt-alternative-secret".to_owned()),
                ..JwtConfig::default()
            },
            ..SecurityConfig::default()
        };
        assert_eq!(
            config_with_jwt.validate(),
            Err("the bearer token must not be empty or whitespace-only"),
            "a complete JWT authenticator must not preserve blank token {token:?}"
        );
    }
}

#[test]
fn application_bearer_validation_preserves_valid_token_exactly() {
    let config = SecurityConfig {
        enabled: true,
        bearer_token: Some("valid-token-sentinel".to_owned()),
        ..SecurityConfig::default()
    };

    assert_eq!(config.validate(), Ok(()));
    assert_eq!(config.bearer_token.as_deref(), Some("valid-token-sentinel"));

    #[cfg(feature = "jwt")]
    let jwt_alternative = SecurityConfig {
        enabled: true,
        jwt: JwtConfig {
            enabled: true,
            secret: Some("jwt-alternative-secret".to_owned()),
            ..JwtConfig::default()
        },
        ..SecurityConfig::default()
    };
    #[cfg(feature = "jwt")]
    assert_eq!(jwt_alternative.validate(), Ok(()));
}

#[test]
fn validation_errors_do_not_echo_a_configured_bearer_token() {
    let config = SecurityConfig {
        enabled: true,
        bearer_token: Some("validation-bearer-secret-sentinel".to_owned()),
        basic: BasicAuthConfig {
            username: Some(String::new()),
            password: Some("basic-password".to_owned()),
        },
        ..SecurityConfig::default()
    };

    let error = config
        .validate()
        .expect_err("invalid Basic configuration should fail validation");
    let diagnostic = format!("{error} {config:?}");
    assert_eq!(error, "basic credentials must not be empty");
    assert!(!diagnostic.contains("validation-bearer-secret-sentinel"));
}

#[test]
fn credential_config_debug_output_redacts_all_credential_values() {
    let config = SecurityConfig {
        enabled: true,
        bearer_token: Some("bearer-sentinel-secret".to_owned()),
        basic: BasicAuthConfig {
            username: Some("basic-user-sentinel".to_owned()),
            password: Some("basic-password-sentinel".to_owned()),
        },
        jwt: JwtConfig {
            enabled: true,
            secret: Some("jwt-secret-sentinel".to_owned()),
            issuer_uri: Some("https://issuer.test/?client_secret=issuer-uri-sentinel".to_owned()),
            jwk_set_uri: Some("https://keys.test/?token=jwks-uri-sentinel".to_owned()),
            ..JwtConfig::default()
        },
        ..SecurityConfig::default()
    };

    let diagnostic = format!("{config:?}");
    for sentinel in [
        "bearer-sentinel-secret",
        "basic-user-sentinel",
        "basic-password-sentinel",
        "jwt-secret-sentinel",
        "issuer-uri-sentinel",
        "jwks-uri-sentinel",
    ] {
        assert!(!diagnostic.contains(sentinel));
    }
    assert!(diagnostic.contains("[REDACTED]"));
}

#[test]
fn config_serialization_continues_to_omit_basic_and_jwt_secrets() {
    let config = SecurityConfig {
        basic: BasicAuthConfig {
            username: Some("user".to_owned()),
            password: Some(BASIC_PASSWORD_SECRET.to_owned()),
        },
        jwt: JwtConfig {
            secret: Some(JWT_SECRET.to_owned()),
            ..JwtConfig::default()
        },
        ..SecurityConfig::default()
    };

    let serialized = serde_json::to_string(&config).expect("config should serialize");
    assert!(!serialized.contains(BASIC_PASSWORD_SECRET));
    assert!(!serialized.contains(JWT_SECRET));
}

#[cfg(feature = "jwt")]
#[test]
fn enabled_jwt_rejects_blank_required_scopes_without_disclosing_credentials() {
    for scope in ["", " ", "\t", " \r\n "] {
        let config = SecurityConfig {
            jwt: JwtConfig {
                enabled: true,
                secret: Some("scope-validation-secret".to_owned()),
                required_scopes: vec![scope.to_owned()],
                ..Default::default()
            },
            ..Default::default()
        };
        let error = config
            .validate()
            .expect_err("blank scope must fail before serving");
        assert_eq!(
            error,
            "JWT required_scopes must not contain empty or whitespace-only values"
        );
        assert!(!format!("{error} {config:?}").contains("scope-validation-secret"));
    }
    for scopes in [vec![], vec!["profile.read".to_owned()]] {
        let config = SecurityConfig {
            jwt: JwtConfig {
                enabled: true,
                secret: Some("scope-validation-secret".to_owned()),
                required_scopes: scopes,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }
}

#[test]
fn incomplete_basic_intent_is_not_silently_ignored_when_bearer_is_usable() {
    for basic in [
        BasicAuthConfig {
            username: Some("username-sentinel".into()),
            password: None,
        },
        BasicAuthConfig {
            username: None,
            password: Some("password-sentinel".into()),
        },
    ] {
        let config = SecurityConfig {
            enabled: true,
            bearer_token: Some("bearer-sentinel".into()),
            basic,
            ..Default::default()
        };
        assert_eq!(
            config.validate().unwrap_err(),
            "basic authentication requires both username and password"
        );
    }
}
