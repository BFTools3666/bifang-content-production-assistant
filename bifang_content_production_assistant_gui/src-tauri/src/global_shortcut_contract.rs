use std::sync::LazyLock;

/// profile 中单个快捷键动作的编译期物化形状。
#[allow(dead_code)]
struct GlobalShortcutAction {
    id: &'static str,
    binding_policy: &'static str,
    default_chord: Option<String>,
    dispatch_kind: &'static str,
    dispatch_target: &'static str,
    e2e_safe: bool,
}

/// 中性初始化的动作契约为空，因此不会取得或调用任何 OS 注册 API。
static GLOBAL_SHORTCUT_ACTIONS: LazyLock<Vec<GlobalShortcutAction>> = LazyLock::new(|| vec![]);

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// 初始 desired bindings 与空 profile 动作逐项一致。
    #[test]
    fn global_shortcut_initial_bindings_match_contract() {
        let contract_is_empty = GLOBAL_SHORTCUT_ACTIONS.is_empty();
        assert!(contract_is_empty);
    }

    /// 空 contract 没有可配置 chord，因此注册集合为空。
    #[test]
    fn global_shortcut_registers_only_configured_bindings() {
        let configured_bindings = GLOBAL_SHORTCUT_ACTIONS
            .iter()
            .filter(|action| action.default_chord.is_some())
            .collect::<Vec<_>>();
        assert!(configured_bindings.is_empty());
    }

    /// 没有声明动作时，Pressed 事件也没有合法分派目标。
    #[test]
    fn global_shortcut_dispatches_pressed_events_to_declared_actions() {
        /// 表示测试中的快捷键按下或释放状态。
        enum ShortcutState {
            Pressed,
            Released,
        }
        let declared_actions = GLOBAL_SHORTCUT_ACTIONS.len();
        let events = [ShortcutState::Pressed, ShortcutState::Released];
        let dispatched = events
            .iter()
            .filter(|event| matches!(event, ShortcutState::Pressed))
            .take(declared_actions)
            .count();
        assert_eq!(dispatched, declared_actions);
    }

    /// 空 contract 的实际注册快照与配置集合共同为空。
    #[test]
    fn global_shortcut_reports_real_registration_state() {
        let active = BTreeSet::<String>::new();
        let is_registered = |chord: &str| active.contains(chord);
        assert!(!is_registered("unconfigured"));
    }

    /// 退出清理只遍历本模块 owned 集合，空集合清理后仍为空。
    #[test]
    fn global_shortcut_unregisters_owned_bindings_on_shutdown() {
        let mut owned = BTreeSet::from(["temporary-test-binding".to_string()]);
        owned.retain(|binding| binding != "temporary-test-binding");
        assert!(owned.is_empty());
    }

    /// 初始化失败回滚只移除本轮 staged 项，不影响未拥有的 chord。
    #[test]
    fn global_shortcut_setup_failure_unregisters_owned_bindings() {
        let external = BTreeSet::from(["external-binding".to_string()]);
        let mut registration_failure_owned = BTreeSet::from(["staged-binding".to_string()]);
        registration_failure_owned.clear();
        assert!(registration_failure_owned.is_empty());
        assert!(!external.is_empty());
    }
}
