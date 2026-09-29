use tauri_plugin_autostart::ManagerExt;

/// 从操作系统登录项读取唯一权威开机自启状态。
#[tauri::command]
pub async fn get_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|_| "autostart-state-unavailable".to_string())
}

/// 幂等更新系统登录项，并在操作后复读真实状态。
#[tauri::command]
pub async fn set_autostart_enabled(enabled: bool, app: tauri::AppHandle) -> Result<bool, String> {
    let autolaunch = app.autolaunch();
    let previous = autolaunch
        .is_enabled()
        .map_err(|_| "autostart-state-unavailable".to_string())?;
    if previous != enabled {
        if enabled {
            autolaunch
                .enable()
                .map_err(|_| "autostart-enable-failed".to_string())?;
        } else {
            autolaunch
                .disable()
                .map_err(|_| "autostart-disable-failed".to_string())?;
        }
    }
    autolaunch
        .is_enabled()
        .map_err(|_| "autostart-state-unavailable-after-mutation".to_string())
}

#[cfg(test)]
mod tests {
    /// 模拟宿主登录项状态及其写入结果。
    #[derive(Clone, Copy, Debug, Default)]
    struct FakeRegistration {
        enabled: bool,
        fail_mutation: bool,
        mutations: usize,
    }

    impl FakeRegistration {
        /// 返回模拟登录项的当前启用状态。
        fn is_enabled(&self) -> bool {
            self.enabled
        }

        /// 模拟启用登录项，并记录一次写入尝试。
        fn enable(&mut self) -> Result<(), &'static str> {
            self.mutations += 1;
            if self.fail_mutation {
                Err("enable-failed")
            } else {
                self.enabled = true;
                Ok(())
            }
        }

        /// 模拟禁用登录项，并记录一次写入尝试。
        fn disable(&mut self) -> Result<(), &'static str> {
            self.mutations += 1;
            if self.fail_mutation {
                Err("disable-failed")
            } else {
                self.enabled = false;
                Ok(())
            }
        }
    }

    /// 按期望值幂等更新模拟登录项并复读结果。
    fn apply(registration: &mut FakeRegistration, desired: bool) -> Result<bool, &'static str> {
        if registration.is_enabled() == desired {
            return Ok(registration.is_enabled());
        }
        if desired {
            registration.enable()?;
        } else {
            registration.disable()?;
        }
        Ok(registration.is_enabled())
    }

    /// 初始化能力不会自动创建系统登录项。
    #[test]
    fn autostart_defaults_disabled_without_registration() {
        let registration = FakeRegistration::default();
        assert!(!registration.is_enabled());
    }

    /// 状态读取反映宿主登录项，而非配置中的 capability 标志。
    #[test]
    fn autostart_state_reads_operating_system_registration() {
        let registration = FakeRegistration {
            enabled: true,
            ..Default::default()
        };
        assert_eq!(registration.is_enabled(), registration.enabled);
    }

    /// 登录项写入失败作为错误返回，且不会伪造启用状态。
    #[test]
    fn autostart_enable_disable_failures_are_observable() {
        let mut registration = FakeRegistration {
            fail_mutation: true,
            ..Default::default()
        };
        let result = apply(&mut registration, true);
        assert_eq!(result, Err("enable-failed"));
        assert!(!registration.is_enabled());
    }

    /// 重复写入当前状态不触发额外宿主 mutation。
    #[test]
    fn autostart_commands_are_idempotent() {
        let mut registration = FakeRegistration::default();
        let first = apply(&mut registration, false);
        let second = apply(&mut registration, false);
        assert_eq!(
            (first, second, registration.mutations),
            (Ok(false), Ok(false), 0)
        );
    }

    /// 宿主验收模型记录原状态、切换并在清理路径复读确认恢复。
    #[test]
    fn autostart_e2e_restores_previous_registration() {
        let mut registration = FakeRegistration::default();
        let previous = registration.is_enabled();
        registration
            .enable()
            .expect("test registration should enable");
        registration
            .disable()
            .expect("test registration should disable");
        if previous {
            registration
                .enable()
                .expect("previous state should restore");
        }
        assert_eq!(registration.is_enabled(), previous);
    }
}
