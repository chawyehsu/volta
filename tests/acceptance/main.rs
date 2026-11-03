use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "mock-network")] {
        mod support;

        // test files
        mod corrupted_download;
        mod direct_install;
        mod direct_uninstall;
        mod execute_binary;
        mod fetch;
        mod hooks;
        mod merged_platform;
        mod migrations;
        mod run_shim_directly;
        #[cfg(not(target_env = "musl"))]
        mod self_update;
        mod setup;
        mod verbose_errors;
        mod which;
        mod volta_bypass;
        mod volta_install;
        mod volta_list;
        mod volta_pin;
        mod volta_run;
        mod volta_uninstall;
    }
}
