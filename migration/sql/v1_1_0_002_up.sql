alter Table `job`
ADD COLUMN `completed_callback` JSON DEFAULT NULL COMMENT '任务完成回调';

ALTER TABLE `job_exec_history` ADD COLUMN `run_id` VARCHAR(50) NOT NULL DEFAULT '' COMMENT '任务运行id';