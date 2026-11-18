# System BLE

仅查询系统当前已连接且匹配服务 UUID 的设备，返回 `address`、`name`。
不扫描广播、不负责连接或特征读写；连接仍由 `tauri-plugin-blec` 负责。

- Windows、Linux、macOS、iOS：使用 btleplug 的 service-only `retrieve_peripherals`。
- Android：使用 `BluetoothManager.getConnectedDevices(GATT)`，按系统缓存的服务 UUID 筛选。
  需要蓝牙连接权限；服务 UUID 缓存缺失时不会返回该设备。
- Apple 平台的 `address` 是设备 UUID，其余平台使用蓝牙地址，格式与 blec 相同。

查询结果不会自动加入另一个 BLE 客户端的内部设备索引。
上游 blec 可重连本次运行中已发现的设备；未被它发现且不再广播的设备仍受其连接入口限制。
