use async_trait::async_trait;
use casbin::{error::AdapterError, Adapter, Error as CasbinError, Filter, Model, Result};
use sea_orm::ConnectionTrait;

use crate::{
    action::{self, Rule, RuleWithType},
    entity, migration,
};

pub struct SeaOrmAdapter<C> {
    conn: C,
    is_filtered: bool,
}

impl<C: ConnectionTrait> SeaOrmAdapter<C> {
    pub async fn new(conn: C) -> Result<Self> {
        match migration::up(&conn).await {
            Ok(_) => Ok(Self {
                conn,
                is_filtered: false,
            }),
            Err(err) => Err(CasbinError::from(AdapterError(Box::new(err)))),
        }
    }
}

impl<C> SeaOrmAdapter<C> {
    fn transform_policy_line<'a>(ptype: &'a str, rule: &'a [String]) -> Option<RuleWithType<'a>> {
        if ptype.trim().is_empty() || rule.is_empty() {
            return None;
        }

        Some(RuleWithType::from_rule(ptype, Rule::from_string(rule)))
    }

    fn normalize_policy(model: &entity::Model) -> Option<Vec<String>> {
        let mut policy = vec![
            &model.v0, &model.v1, &model.v2, &model.v3, &model.v4, &model.v5,
        ];

        loop {
            match policy.last() {
                Some(last) if last.is_empty() => {
                    policy.pop();
                }
                _ => break,
            }
        }

        if policy.is_empty() {
            None
        } else {
            Some(policy.iter().map(|&x| x.to_owned()).collect())
        }
    }
}

#[async_trait]
impl<C: ConnectionTrait + Send + Sync> Adapter for SeaOrmAdapter<C> {
    async fn load_policy(&mut self, m: &mut dyn Model) -> Result<()> {
        let rules = action::load_policy(&self.conn).await?;

        for rule in &rules {
            let Some(sec) = rule.ptype.chars().next().map(|x| x.to_string()) else {
                continue;
            };
            let Some(t1) = m.get_mut_model().get_mut(&sec) else {
                continue;
            };
            let Some(t2) = t1.get_mut(&rule.ptype) else {
                continue;
            };
            let Some(policy) = Self::normalize_policy(rule) else {
                continue;
            };
            t2.get_mut_policy().insert(policy);
        }

        Ok(())
    }

    async fn load_filtered_policy<'a>(&mut self, m: &mut dyn Model, f: Filter<'a>) -> Result<()> {
        let rules = action::load_filtered_policy(&self.conn, f).await?;
        self.is_filtered = true;

        for rule in &rules {
            let Some(sec) = rule.ptype.chars().next().map(|x| x.to_string()) else {
                continue;
            };
            let Some(t1) = m.get_mut_model().get_mut(&sec) else {
                continue;
            };
            let Some(t2) = t1.get_mut(&rule.ptype) else {
                continue;
            };
            let Some(policy) = Self::normalize_policy(rule) else {
                continue;
            };
            t2.get_mut_policy().insert(policy);
        }

        Ok(())
    }

    async fn save_policy(&mut self, m: &mut dyn Model) -> Result<()> {
        let mut rules = Vec::new();

        if let Some(map) = m.get_model().get("p") {
            for (ptype, assertion) in map {
                let new_rules = assertion
                    .get_policy()
                    .into_iter()
                    .filter_map(|x| Self::transform_policy_line(ptype, x));

                rules.extend(new_rules);
            }
        }

        if let Some(map) = m.get_model().get("g") {
            for (ptype, assertion) in map {
                let new_rules = assertion
                    .get_policy()
                    .into_iter()
                    .filter_map(|x| Self::transform_policy_line(ptype, x));

                rules.extend(new_rules);
            }
        }

        action::save_policies(&self.conn, rules).await
    }

    async fn clear_policy(&mut self) -> Result<()> {
        action::clear_policy(&self.conn).await
    }

    fn is_filtered(&self) -> bool {
        self.is_filtered
    }

    async fn add_policy(&mut self, _sec: &str, ptype: &str, rule: Vec<String>) -> Result<bool> {
        let Some(rule_with_type) = Self::transform_policy_line(ptype, rule.as_slice()) else {
            return Ok(false);
        };

        action::add_policy(&self.conn, rule_with_type).await
    }

    async fn add_policies(
        &mut self,
        _sec: &str,
        ptype: &str,
        rules: Vec<Vec<String>>,
    ) -> Result<bool> {
        let rules = rules
            .iter()
            .filter_map(|x| Self::transform_policy_line(ptype, x))
            .collect::<Vec<_>>();

        if rules.is_empty() {
            return Ok(false);
        }

        action::add_policies(&self.conn, rules).await
    }

    async fn remove_policy(&mut self, _sec: &str, ptype: &str, rule: Vec<String>) -> Result<bool> {
        let Some(rule_with_type) = Self::transform_policy_line(ptype, rule.as_slice()) else {
            return Ok(false);
        };

        action::remove_policy(&self.conn, rule_with_type).await
    }

    async fn remove_policies(
        &mut self,
        _sec: &str,
        ptype: &str,
        rules: Vec<Vec<String>>,
    ) -> Result<bool> {
        let rules = rules
            .iter()
            .filter_map(|x| Self::transform_policy_line(ptype, x))
            .collect::<Vec<_>>();

        if rules.is_empty() {
            return Ok(false);
        }

        action::remove_policies(&self.conn, rules).await
    }

    async fn remove_filtered_policy(
        &mut self,
        _sec: &str,
        ptype: &str,
        field_index: usize,
        field_values: Vec<String>,
    ) -> Result<bool> {
        if field_index <= 5 && !field_values.is_empty() && field_values.len() + field_index <= 6 {
            let rule = Rule::from_string(&field_values);
            action::remove_filtered_policy(&self.conn, ptype, field_index, rule).await
        } else {
            Ok(false)
        }
    }
}

// Copy from https://github.com/casbin-rs/sqlx-adapter/blob/master/src/adapter.rs
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use casbin::Adapter;
    use sea_orm::{ConnectOptions, Database};

    use crate::adapter::SeaOrmAdapter;

    fn to_owned(v: Vec<&str>) -> Vec<String> {
        v.into_iter().map(|x| x.to_owned()).collect()
    }

    #[cfg_attr(
        any(feature = "runtime-tokio-native-tls", feature = "runtime-tokio-rustls"),
        tokio::test(flavor = "multi_thread")
    )]
    async fn test_adapter() {
        use casbin::prelude::*;

        let file_adapter = FileAdapter::new("examples/rbac_policy.csv");

        let m = DefaultModel::from_file("examples/rbac_model.conf")
            .await
            .unwrap();

        let mut e = Enforcer::new(m, file_adapter).await.unwrap();
        let db_url = {
            #[cfg(feature = "postgres")]
            {
                "postgres://root:123456@localhost:5432/casbin"
            }

            #[cfg(feature = "mysql")]
            {
                "mysql://root:123456@localhost:3306/casbin"
            }

            #[cfg(feature = "sqlite")]
            {
                "sqlite:casbin.db"
            }
        };

        let mut opt = ConnectOptions::new(db_url.to_owned());
        opt.max_connections(100)
            .min_connections(5)
            .connect_timeout(Duration::from_secs(8))
            .acquire_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(8));

        let db = Database::connect(opt).await.unwrap();

        let mut adapter = SeaOrmAdapter::new(db).await.unwrap();

        assert!(adapter.save_policy(e.get_mut_model()).await.is_ok());

        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["alice", "data1", "read"]))
            .await
            .unwrap());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["bob", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["data2_admin", "data2", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["data2_admin", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_ok());

        assert!(adapter
            .add_policy("", "p", to_owned(vec!["alice", "data1", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["bob", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["data2_admin", "data2", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["data2_admin", "data2", "write"]))
            .await
            .is_ok());

        assert!(adapter
            .remove_policies(
                "",
                "p",
                vec![
                    to_owned(vec!["alice", "data1", "read"]),
                    to_owned(vec!["bob", "data2", "write"]),
                    to_owned(vec!["data2_admin", "data2", "read"]),
                    to_owned(vec!["data2_admin", "data2", "write"]),
                ]
            )
            .await
            .is_ok());

        assert!(adapter
            .add_policies(
                "",
                "p",
                vec![
                    to_owned(vec!["alice", "data1", "read"]),
                    to_owned(vec!["bob", "data2", "write"]),
                    to_owned(vec!["data2_admin", "data2", "read"]),
                    to_owned(vec!["data2_admin", "data2", "write"]),
                ]
            )
            .await
            .is_ok());

        assert!(adapter
            .add_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_ok());

        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["alice", "data1", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["bob", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["data2_admin", "data2", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["data2_admin", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_ok());

        assert!(!adapter
            .remove_policy(
                "",
                "g",
                to_owned(vec!["alice", "data2_admin", "not_exists"])
            )
            .await
            .unwrap());

        assert!(adapter
            .add_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_err());

        assert!(!adapter
            .remove_filtered_policy(
                "",
                "g",
                0,
                to_owned(vec!["alice", "data2_admin", "not_exists"]),
            )
            .await
            .unwrap());

        assert!(adapter
            .remove_filtered_policy("", "g", 0, to_owned(vec!["alice", "data2_admin"]))
            .await
            .unwrap());

        assert!(adapter
            .add_policy(
                "",
                "g",
                to_owned(vec!["alice", "data2_admin", "domain1", "domain2"]),
            )
            .await
            .is_ok());
        assert!(adapter
            .remove_filtered_policy(
                "",
                "g",
                1,
                to_owned(vec!["data2_admin", "domain1", "domain2"]),
            )
            .await
            .unwrap());

        // GitHub issue: https://github.com/casbin-rs/sqlx-adapter/issues/64
        assert!(adapter
            .add_policy("", "g", to_owned(vec!["carol", "data1_admin"]),)
            .await
            .is_ok());
        assert!(adapter
            .remove_filtered_policy("", "g", 0, to_owned(vec!["carol"]),)
            .await
            .unwrap());
        assert_eq!(Vec::<String>::new(), e.get_roles_for_user("carol", None));

        // GitHub issue: https://github.com/casbin-rs/sqlx-adapter/pull/90
        // add policies:
        // p, alice_rfp, book_rfp, read_rfp
        // p, bob_rfp, book_rfp, read_rfp
        // p, bob_rfp, book_rfp, write_rfp
        // p, alice_rfp, pen_rfp, get_rfp
        // p, bob_rfp, pen_rfp, get_rfp
        // p, alice_rfp, pencil_rfp, get_rfp
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["alice_rfp", "book_rfp", "read_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["bob_rfp", "book_rfp", "read_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["bob_rfp", "book_rfp", "write_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["alice_rfp", "pen_rfp", "get_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["bob_rfp", "pen_rfp", "get_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy(
                "",
                "p",
                to_owned(vec!["alice_rfp", "pencil_rfp", "get_rfp"]),
            )
            .await
            .is_ok());

        // should remove (return true) all policies where "book_rfp" is in the second position
        assert!(adapter
            .remove_filtered_policy("", "p", 1, to_owned(vec!["book_rfp"]),)
            .await
            .unwrap());

        // should remove (return true) all policies which match "alice_rfp" on first position
        // and "get_rfp" on third position
        assert!(adapter
            .remove_filtered_policy("", "p", 0, to_owned(vec!["alice_rfp", "", "get_rfp"]),)
            .await
            .unwrap());

        // shadow the previous enforcer
        let mut e = Enforcer::new(
            "examples/rbac_with_domains_model.conf",
            "examples/rbac_with_domains_policy.csv",
        )
        .await
        .unwrap();

        assert!(adapter.save_policy(e.get_mut_model()).await.is_ok());
        e.set_adapter(adapter).await.unwrap();

        let filter = Filter {
            p: vec!["", "domain1"],
            g: vec!["", "", "domain1"],
        };

        e.load_filtered_policy(filter).await.unwrap();
        assert!(e.enforce(("alice", "domain1", "data1", "read")).unwrap());
        assert!(e.enforce(("alice", "domain1", "data1", "write")).unwrap());
        assert!(!e.enforce(("alice", "domain1", "data2", "read")).unwrap());
        assert!(!e.enforce(("alice", "domain1", "data2", "write")).unwrap());
        assert!(!e.enforce(("bob", "domain2", "data2", "read")).unwrap());
        assert!(!e.enforce(("bob", "domain2", "data2", "write")).unwrap());
    }
}
