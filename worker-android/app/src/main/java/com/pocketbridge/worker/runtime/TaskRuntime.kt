package com.pocketbridge.worker.runtime

import kotlinx.coroutines.Job
import java.util.concurrent.ConcurrentHashMap

class TaskRuntime {
    private val activeJobs = ConcurrentHashMap<String, Job>()

    fun registerJob(jobId: String, job: Job) {
        activeJobs[jobId] = job
    }

    fun unregisterJob(jobId: String) {
        activeJobs.remove(jobId)
    }

    fun cancelJob(jobId: String): Boolean {
        val job = activeJobs.remove(jobId) ?: return false
        job.cancel()
        return true
    }

    fun hasActiveJob(jobId: String): Boolean = activeJobs.containsKey(jobId)
}
