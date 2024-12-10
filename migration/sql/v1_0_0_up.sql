-- Active: 1717845081831@@127.0.0.1@3306@jiascheduler
DROP TABLE IF EXISTS `user`;

CREATE TABLE `user` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `user_id` varchar(10) not null COMMENT '用户id',
    `username` varchar(50) NOT NULL DEFAULT '' COMMENT '用户名',
    `nickname` varchar(50) NOT NULL DEFAULT '' COMMENT '昵称',
    `is_root` BOOLEAN NOT NULL DEFAULT false COMMENT '是否是根用户',
    `role_id` bigint(20) unsigned not null DEFAULT 0 COMMENT '用户角色',
    `salt` varchar(50) NOT NULL DEFAULT '' COMMENT '密码加盐',
    `password` varchar(100) NOT NULL DEFAULT '' COMMENT '密码',
    `avatar` VARCHAR(100) NOT NULL DEFAULT '' COMMENT '头像',
    `email` VARCHAR(200) NOT NULL DEFAULT '' COMMENT '邮箱',
    `phone` VARCHAR(20) NOT NULL DEFAULT '' COMMENT '电话',
    `gender` VARCHAR(10) NOT NULL DEFAULT 'male' COMMENT '性别 male 男 female 女 ...',
    `introduction` VARCHAR(2000) NOT NULL DEFAULT '' COMMENT '介绍',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uqe_username` (`username`),
    UNIQUE KEY `uqe_user_id` (`user_id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '用户角色';

DROP TABLE IF EXISTS `instance`;

CREATE TABLE `instance` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `instance_id` varchar(40) NOT NULL DEFAULT '' COMMENT '实例id',
    `ip` varchar(100) NOT NULL DEFAULT '' COMMENT '节点ip',
    `namespace` varchar(100) NOT NULL DEFAULT 'default' COMMENT 'namespace',
    `mac_addr` CHAR(20) NOT NULL DEFAULT '' COMMENT 'mac地址',
    `instance_group_id` BIGINT(20) unsigned NOT NULL DEFAULT '0' COMMENT '实例分组',
    `info` VARCHAR(500) NOT NULL DEFAULT '' COMMENT '介绍',
    `status` tinyint(4) NOT NULL DEFAULT '0' COMMENT '节点状态: 0下线, 1上线',
    `sys_user` VARCHAR(20) NOT NULL DEFAULT 'root' COMMENT '系统用户',
    `password` VARCHAR(1000) NOT NULL DEFAULT '' COMMENT '密码',
    `ssh_port` SMALLINT UNSIGNED NOT NULL DEFAULT '22' COMMENT 'ssh 连接端口',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_ip` (`mac_addr`, `ip`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '节点';

DROP TABLE IF EXISTS `instance_group`;

CREATE TABLE `instance_group` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '分组名称',
    `info` varchar(200) NOT NULL DEFAULT '' COMMENT '介绍',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`name`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '实例分组';

DROP TABLE IF EXISTS `instance_role`;

CREATE TABLE `instance_role` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `role_id` BIGINT(20) unsigned NOT NULL DEFAULT '0' COMMENT '角色id',
    `instance_id` varchar(40) NOT NULL DEFAULT '' COMMENT '实例id',
    `instance_group_id` BIGINT(20) unsigned NOT NULL DEFAULT '0' COMMENT '实例分组id',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_instance` (
        `role_id`,
        `instance_id`,
        `instance_group_id`
    )
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '实例角色';

DROP TABLE IF EXISTS `role`;

CREATE TABLE `role` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '角色名称',
    `info` varchar(200) NOT NULL DEFAULT '' COMMENT '介绍',
    `is_admin` BOOLEAN NOT NULL DEFAULT false COMMENT '是否是超级管理员',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`name`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '角色';

DROP TABLE IF EXISTS `user_server`;

CREATE TABLE `user_server` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `user_id` varchar(10) not null COMMENT '用户id',
    `instance_id` varchar(40) NOT NULL DEFAULT '' COMMENT '实例id',
    `instance_group_id` BIGINT(20) NOT NULL DEFAULT '0' COMMENT '实例分组id',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_user_instance` (
        `user_id`,
        `instance_id`,
        `instance_group_id`
    )
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '节点';

DROP TABLE IF EXISTS `job_timer`;

CREATE TABLE `job_timer` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '定时器名称',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `timer_expr` JSON NULL COMMENT '定时表达式',
    `job_type` VARCHAR(50) NOT NULL DEFAULT 'default' COMMENT '作业类型',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    KEY `idx_eid` (`eid`),
    UNIQUE KEY `uk_name` (`name`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '作业定时器';

DROP TABLE IF EXISTS `job`;

CREATE TABLE `job` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `executor_id` BIGINT UNSIGNED NOT NULL DEFAULT 0 COMMENT '执行器',
    `job_type` VARCHAR(50) NOT NULL DEFAULT 'default' COMMENT '作业类型',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '作业名称',
    `code` text NOT NULL COMMENT '代码',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `bundle_script` JSON DEFAULT NULL COMMENT '作业脚本包',
    `upload_file` VARCHAR(500) NOT NULL DEFAULT '' COMMENT '上传文件地址',
    `work_dir` VARCHAR(500) NOT NULL DEFAULT '' COMMENT '工作目录',
    `work_user` VARCHAR(50) NOT NULL DEFAULT '' COMMENT '执行用户',
    `timeout` BIGINT UNSIGNED NOT NULL DEFAULT 60 COMMENT '执行超时,单位秒',
    `max_retry` TINYINT UNSIGNED NOT NULL DEFAULT 1 COMMENT '最大重试次数',
    `max_parallel` TINYINT UNSIGNED NOT NULL DEFAULT 1 COMMENT '进程最大并行数',
    `is_public` tinyint(1) NOT NULL DEFAULT '0' COMMENT '是否公开',
    `display_on_dashboard` BOOLEAN NOT NULL DEFAULT false COMMENT '是否显示在仪表盘',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `args` json DEFAULT NULL COMMENT '作业参数',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`name`, `created_user`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '用户作业';

DROP TABLE IF EXISTS `job_bundle_script`;

CREATE TABLE `job_bundle_script` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `executor_id` BIGINT UNSIGNED NOT NULL DEFAULT 0 COMMENT '执行器',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '作业名称',
    `code` text NOT NULL COMMENT '代码',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `args` json DEFAULT NULL COMMENT '参数',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`created_user`, `name`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '供作业批量执行的脚本';

DROP TABLE IF EXISTS `executor`;

CREATE TABLE `executor` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '作业名称',
    `command` varchar(100) NOT NULL DEFAULT '' COMMENT '执行命令',
    `platform` varchar(10) NOT NULL DEFAULT 'linux' COMMENT '操作平台',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `read_code_from_stdin` BOOLEAN NOT NULL DEFAULT false COMMENT '是否从stdin读入代码',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`name`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '作业执行类型';

INSERT INTO
    executor (
        `name`,
        `command`,
        `platform`,
        `info`,
        `created_user`,
        `updated_user`
    )
VALUES
    (
        'bash',
        'bash -c',
        'linux',
        'run linux bash sciript',
        'system',
        'system'
    ),
    (
        'python',
        'python -c',
        'linux',
        'run python script',
        'system',
        'system'
    );

DROP TABLE IF EXISTS `job_exec_history`;

CREATE TABLE `job_exec_history` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `schedule_id` varchar(40) NOT NULL DEFAULT '' COMMENT '调度uuid',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `job_type` VARCHAR(50) NOT NULL DEFAULT 'default' COMMENT '作业类型',
    `instance_id` varchar(40) NOT NULL DEFAULT '' COMMENT '实例id',
    `bundle_script_result` JSON DEFAULT NULL COMMENT '脚本包执行结果',
    `exit_status` varchar(200) NOT NULL DEFAULT '' COMMENT '退出状态',
    `exit_code` int NOT NULL DEFAULT 0 COMMENT '退出码',
    `output` text NOT NULL COMMENT '执行输出',
    `start_time` timestamp NULL DEFAULT NULL COMMENT 'job开始执行时间',
    `end_time` timestamp NULL DEFAULT NULL COMMENT 'job结束时间',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    KEY `idx_eid` (`eid`),
    KEY `idx_ip` (`instance_id`),
    KEY `idx_schedule_id` (`schedule_id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '作业执行历史';

DROP TABLE IF EXISTS `job_running_status`;

CREATE TABLE `job_running_status` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `instance_id` varchar(40) NOT NULL DEFAULT '' COMMENT '实例id',
    `schedule_type` VARCHAR(10) NOT NULL DEFAULT 'once' COMMENT '调度类型 once timer flow',
    `job_type` VARCHAR(50) NOT NULL DEFAULT 'default' COMMENT '作业类型',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `schedule_id` varchar(40) NOT NULL DEFAULT '' COMMENT '调度id',
    `schedule_status` varchar(40) NOT NULL DEFAULT '' COMMENT '调度状态 scheduling stop',
    `run_status` VARCHAR(40) NOT NULL DEFAULT '' COMMENT '运行状态 running stop',
    `exit_status` varchar(200) NOT NULL DEFAULT '' COMMENT '退出状态',
    `exit_code` int NOT NULL DEFAULT 0 COMMENT '退出码',
    `dispatch_result` json DEFAULT NULL COMMENT '派送结果',
    `start_time` timestamp NULL DEFAULT NULL COMMENT 'job开始执行时间',
    `end_time` timestamp NULL DEFAULT NULL COMMENT 'job结束时间',
    `next_time` timestamp NULL DEFAULT NULL COMMENT '下次执行时间',
    `prev_time` timestamp NULL DEFAULT NULL COMMENT '上次执行时间',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_eid` (
        `eid`,
        `schedule_type`,
        `instance_id`
    )
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '作业运行状态';

DROP TABLE IF EXISTS `job_schedule_history`;

CREATE TABLE `job_schedule_history` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `schedule_id` varchar(40) NOT NULL DEFAULT '' COMMENT '调度id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '调度名称',
    `job_type` VARCHAR(50) NOT NULL DEFAULT 'default' COMMENT '作业类型 default, bundle',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `dispatch_result` json DEFAULT NULL COMMENT '调度派送结果',
    `schedule_type` varchar(20) NOT NULL DEFAULT '' COMMENT '调度类型 once flow timer daemon',
    `action` varchar(20) NOT NULL DEFAULT '' COMMENT '动作 exec kill start_timer stop_timer',
    `dispatch_data` json DEFAULT NULL COMMENT '调度派送数据',
    `snapshot_data` json DEFAULT NULL COMMENT '快照数据',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    KEY `idx_schedule_id` (`schedule_id`),
    KEY `idx_eid` (`eid`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '作业调度历史';