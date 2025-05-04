pub const SQL: &'static str = r#"
CREATE DATABASE IF NOT EXISTS `jiascheduler` CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci;

DROP TABLE IF EXISTS `user`;

CREATE TABLE `user` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `user_id` varchar(10) not null COMMENT '用户id',
    `username` varchar(50) NOT NULL DEFAULT '' COMMENT '用户名',
    `nickname` varchar(50) NOT NULL DEFAULT '' COMMENT '昵称',
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

DROP TABLE IF EXISTS `agent_release_version`;

CREATE TABLE `agent_release_version` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '版本名称',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `url` varchar(200) NOT NULL DEFAULT '' COMMENT '下载地址',
    `release_version` varchar(100) NOT NULL DEFAULT '' COMMENT 'agent 版本',
    `release_scope` tinyint(4) NOT NULL DEFAULT '0' COMMENT '0 灰度 1 全量',
    `release_ip` json DEFAULT NULL COMMENT '灰度发布ip',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_version` (`release_version`)
) ENGINE = InnoDB AUTO_INCREMENT = 2 DEFAULT CHARSET = utf8mb4 COMMENT = 'agent版本管理';

DROP TABLE IF EXISTS `instance`;

CREATE TABLE `instance` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `ip` varchar(100) NOT NULL DEFAULT '' COMMENT '节点ip',
    `namespace` varchar(100) NOT NULL DEFAULT 'default' COMMENT 'namespace',
    `instance_group_id` BIGINT(20) unsigned NOT NULL DEFAULT '0' COMMENT '实例分组',
    `info` VARCHAR(500) NOT NULL DEFAULT '' COMMENT '介绍',
    `status` tinyint(4) NOT NULL DEFAULT '0' COMMENT '节点状态: 0下线, 1上线',
    `sys_user` VARCHAR(20) NOT NULL DEFAULT 'root' COMMENT '系统用户',
    `password` VARCHAR(200) NOT NULL DEFAULT '' COMMENT '密码',
    `ssh_port` SMALLINT UNSIGNED NOT NULL DEFAULT '22' COMMENT 'ssh 连接端口',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_ip` (`namespace`, `ip`)
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
    `instance_id` BIGINT(20) unsigned NOT NULL DEFAULT '0' COMMENT '实例id',
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
    `instance_id` bigint(20) NOT NULL DEFAULT '0' COMMENT '实例id',
    `instance_group_id` BIGINT(20) NOT NULL DEFAULT '0' COMMENT '实例分组id',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_user_instance` (`user_id`, `instance_id`),
    UNIQUE KEY `uk_user_instance_group` (
        `user_id`,
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
) ENGINE = InnoDB AUTO_INCREMENT = 8 DEFAULT CHARSET = utf8mb4 COMMENT = '作业定时器';

DROP TABLE IF EXISTS `job`;

CREATE TABLE `job` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `executor_id` int NOT NULL DEFAULT 0 COMMENT '执行器',
    `job_type` VARCHAR(50) NOT NULL DEFAULT 'default' COMMENT '作业类型',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '作业名称',
    `code` text NOT NULL COMMENT '代码',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `bundle_script` JSON DEFAULT NULL COMMENT '作业脚本包',
    `upload_file` VARCHAR(500) NOT NULL DEFAULT '' COMMENT '上传文件地址',
    `is_public` tinyint(1) NOT NULL DEFAULT '0' COMMENT '是否公开',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `args` json DEFAULT NULL COMMENT '作业参数',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`name`)
) ENGINE = InnoDB AUTO_INCREMENT = 8 DEFAULT CHARSET = utf8mb4 COMMENT = '用户作业';

DROP TABLE IF EXISTS `job_bundle_script`;

CREATE TABLE `job_bundle_script` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `executor_id` int NOT NULL DEFAULT 0 COMMENT '执行器',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '作业名称',
    `code` text NOT NULL COMMENT '代码',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `args` json DEFAULT NULL COMMENT '参数',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`name`)
) ENGINE = InnoDB AUTO_INCREMENT = 8 DEFAULT CHARSET = utf8mb4 COMMENT = '供作业批量执行的脚本';

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
VALUES (
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
    `bind_ip` varchar(200) NOT NULL DEFAULT '0' COMMENT 'job绑定的ip',
    `bundle_script_result` JSON DEFAULT NULL COMMENT '脚本包执行结果',
    `exit_status` varchar(200) NOT NULL DEFAULT '' COMMENT '退出状态',
    `exit_code` int NOT NULL DEFAULT 0 COMMENT '退出码',
    `output` text NOT NULL COMMENT '执行输出',
    `start_time` timestamp NULL DEFAULT NULL COMMENT 'job开始执行时间',
    `end_time` timestamp NULL DEFAULT NULL COMMENT 'job结束时间',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`)
) ENGINE = InnoDB AUTO_INCREMENT = 19 DEFAULT CHARSET = utf8mb4 COMMENT = '作业执行历史';

DROP TABLE IF EXISTS `job_organizer`;

CREATE TABLE `job_organizer` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '流程',
    `nodes` json DEFAULT NULL COMMENT '节点',
    `edges` json DEFAULT NULL COMMENT '边线',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `is_public` tinyint(1) NOT NULL DEFAULT '0' COMMENT '是否公开',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`name`, `created_user`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '作业编排';

DROP TABLE IF EXISTS `job_organizer_process`;

CREATE TABLE `job_organizer_process` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '流程名',
    `organizer_id` bigint(20) unsigned NOT NULL DEFAULT '0' COMMENT '编排id',
    `organizer_version` varchar(100) NOT NULL DEFAULT '' COMMENT '版本',
    `process_id` varchar(100) NOT NULL DEFAULT '' COMMENT '流程id',
    `status` varchar(100) NOT NULL DEFAULT 'start_process' COMMENT '流程状态',
    `current_node` varchar(100) NOT NULL DEFAULT '' COMMENT '当前节点',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (`name`, `created_user`),
    KEY `process_id` (`process_id`, `created_user`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '运行中的任务进程';

DROP TABLE IF EXISTS `job_organizer_release`;

CREATE TABLE `job_organizer_release` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `organizer_id` bigint(20) unsigned NOT NULL DEFAULT '0' COMMENT '编排id',
    `version` varchar(100) NOT NULL DEFAULT '' COMMENT '版本',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '版本名称',
    `info` varchar(500) NOT NULL DEFAULT '' COMMENT '描述信息',
    `is_public` tinyint(1) NOT NULL DEFAULT '0' COMMENT '是否公开',
    `nodes` json DEFAULT NULL COMMENT '节点数据',
    `edges` json DEFAULT NULL COMMENT '边线数据',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_name` (
        `name`,
        `organizer_id`,
        `created_user`
    ),
    KEY `idx_version` (`organizer_id`, `version`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '作业编排';

DROP TABLE IF EXISTS `job_organizer_release_edge`;

CREATE TABLE `job_organizer_release_edge` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `version` varchar(100) NOT NULL DEFAULT '' COMMENT '版本',
    `edge_id` varchar(100) NOT NULL DEFAULT '' COMMENT '节点',
    `edge_type` varchar(50) NOT NULL DEFAULT '' COMMENT '节点类型',
    `props` json DEFAULT NULL COMMENT '属性',
    `source_node_id` varchar(100) NOT NULL DEFAULT '' COMMENT '源节点id',
    `target_node_id` varchar(100) NOT NULL DEFAULT '' COMMENT '目标节点id',
    `edge_val` varchar(100) NOT NULL DEFAULT '' COMMENT '边值',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    KEY `idx_version` (`version`),
    KEY `idx_source_node_id` (`source_node_id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '已发布的作业边线集合';

DROP TABLE IF EXISTS `job_organizer_release_node`;

CREATE TABLE `job_organizer_release_node` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `version` varchar(100) NOT NULL DEFAULT '' COMMENT '版本',
    `node_id` varchar(100) NOT NULL DEFAULT '' COMMENT '节点',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '节点名称',
    `node_type` varchar(50) NOT NULL DEFAULT '' COMMENT '节点类型',
    `flow_type` varchar(50) NOT NULL DEFAULT '' COMMENT '流程类型',
    `task_type` varchar(50) NOT NULL DEFAULT '' COMMENT '任务类型',
    `dispatch_data` json DEFAULT NULL COMMENT '发送给执行引擎的可以直接执行的数据',
    `props` json DEFAULT NULL COMMENT '属性',
    `condition` text NOT NULL COMMENT '条件表达式',
    `bind_ip` json DEFAULT NULL COMMENT '绑定的执行节点',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    KEY `idx_version` (`version`, `node_id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '已发布的作业节点集合';

DROP TABLE IF EXISTS `job_organizer_task`;

CREATE TABLE `job_organizer_task` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `process_id` varchar(100) NOT NULL DEFAULT '' COMMENT '流程id',
    `node_id` varchar(100) NOT NULL DEFAULT '' COMMENT '流程节点id',
    `status` varchar(100) NOT NULL DEFAULT '' COMMENT '流程状态',
    `output` varchar(100) NOT NULL DEFAULT '' COMMENT '当为条件节点时,值不为空: true false',
    `bind_total` int(11) NOT NULL DEFAULT '0' COMMENT '绑定的节点数量',
    `restart_num` int(11) NOT NULL DEFAULT '0' COMMENT '重启次数',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_process_node` (`process_id`, `node_id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '任务进程中的各个任务';

DROP TABLE IF EXISTS `job_organizer_task_result`;

CREATE TABLE `job_organizer_task_result` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `process_id` varchar(100) NOT NULL DEFAULT '' COMMENT '流程id',
    `node_id` varchar(100) NOT NULL DEFAULT '' COMMENT '流程节点id',
    `bind_ip` char(20) NOT NULL DEFAULT '' COMMENT '节点ip',
    `exit_code` tinyint(4) NOT NULL DEFAULT '0' COMMENT '退出code',
    `exit_status` varchar(200) NOT NULL DEFAULT '' COMMENT '退出状态',
    `output` text NOT NULL COMMENT '执行输出',
    `status` varchar(100) NOT NULL DEFAULT '' COMMENT '任务状态:start,process,end',
    `restart_num` int(11) NOT NULL DEFAULT '0' COMMENT '重启次数',
    `dispatch_result` json DEFAULT NULL COMMENT '调度派送结果',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_process_node_bind` (
        `process_id`,
        `node_id`,
        `bind_ip`
    )
) ENGINE = InnoDB AUTO_INCREMENT = 15 DEFAULT CHARSET = utf8mb4 COMMENT = '任务执行结果';

DROP TABLE IF EXISTS `job_running_status`;

CREATE TABLE `job_running_status` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `bind_ip` varchar(200) NOT NULL DEFAULT '' COMMENT '绑定的ip',
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
        `bind_ip`
    )
) ENGINE = InnoDB AUTO_INCREMENT = 3 DEFAULT CHARSET = utf8mb4 COMMENT = '作业运行状态';

DROP TABLE IF EXISTS `job_schedule_history`;

CREATE TABLE `job_schedule_history` (
    `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `schedule_id` varchar(40) NOT NULL DEFAULT '' COMMENT '调度id',
    `name` varchar(100) NOT NULL DEFAULT '' COMMENT '调度名称',
    `job_type` VARCHAR(50) NOT NULL DEFAULT 'default' COMMENT '作业类型 default, bundle',
    `eid` varchar(100) NOT NULL DEFAULT '' COMMENT '执行id',
    `dispatch_result` json DEFAULT NULL COMMENT '调度派送结果',
    `schedule_type` varchar(20) NOT NULL DEFAULT '' COMMENT '调度类型 once flow timer',
    `action` varchar(20) NOT NULL DEFAULT '' COMMENT '动作 exec kill start_timer stop_timer',
    `dispatch_data` json DEFAULT NULL COMMENT '调度派送数据',
    `snapshot_data` json DEFAULT NULL COMMENT '快照数据',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `updated_user` varchar(50) NOT NULL DEFAULT '' COMMENT '修改人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    KEY `idx_schedule_id` (`schedule_id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '作业调度历史';

DROP TABLE IF EXISTS `tag`;

CREATE TABLE `tag` (
    `id` bigint unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `tag_key` VARCHAR(100) NOT NULL DEFAULT '' COMMENT '标签键',
    `tag_val` VARCHAR(100) NOT NULL DEFAULT '' COMMENT '标签值',
    `created_user` varchar(50) not null default '' comment '创建人sso user_name',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_kv` (
        `tag_key`,
        `tag_val`,
        `created_user`
    )
) ENGINE = InnoDB COMMENT = '用户标签';

DROP TABLE IF EXISTS `tag_resource`;

CREATE TABLE `tag_resource` (
    `id` bigint unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `tag_id` bigint unsigned not null default 0 comment 'tag id',
    `resource_type` VARCHAR(20) NOT NULL DEFAULT 'ip' COMMENT '资源类型, 默认ip',
    `resource_val` VARCHAR(100) NOT NULL DEFAULT '' COMMENT '资源值',
    `created_user` varchar(50) not null default '' comment '创建人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `updated_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '修改时间',
    PRIMARY KEY (`id`)
) ENGINE = InnoDB COMMENT = '标签绑定的资源';
"#;
