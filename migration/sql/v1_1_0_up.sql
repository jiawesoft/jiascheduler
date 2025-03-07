
DROP TABLE IF EXISTS `tag`;

CREATE TABLE `tag` (
    `id` bigint unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `tag_name` varchar(40) NOT NULL DEFAULT '' COMMENT '调度uuid',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_tag_name` ( `tag_name`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '标签';

DROP TABLE IF EXISTS `tag_resource`;
CREATE TABLE `tag_resource` (
    `id` bigint unsigned NOT NULL AUTO_INCREMENT COMMENT '自增id',
    `tag_id` bigint unsigned NOT NULL DEFAULT 0 COMMENT '标签id',
    `resource_type` varchar(40) NOT NULL DEFAULT '' COMMENT '资源类型',
    `resource_id` BIGINT UNSIGNED NOT NULL DEFAULT 0 COMMENT '标签值id',
    `created_user` varchar(50) NOT NULL DEFAULT '' COMMENT '创建人',
    `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_tag_id` (`resource_type`, `tag_id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COMMENT = '标签绑定';

