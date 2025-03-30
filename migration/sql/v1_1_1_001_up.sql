alter Table `job`
ADD COLUMN `pid` unsigned NOT NULL DEFAULT 0 COMMENT '上级id',
ADD COLUMN `version` VARCHAR(100) NOT NULL DEFAULT '' COMMENT '版本号',
ADD COLUMN `version_name` VARCHAR(100) NOT NULL DEFAULT '' COMMENT '版本名称';

ALTER TABLE `job_exec_history`
ADD COLUMN `run_id` VARCHAR(50) NOT NULL DEFAULT '' COMMENT '任务运行id';
