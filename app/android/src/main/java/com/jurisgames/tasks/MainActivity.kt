package com.jurisgames.tasks

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import com.jurisgames.tasks.ui.TasksApp

class MainActivity : ComponentActivity() {
    private val node get() = (application as TasksApplication).node

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { TasksApp(node) }
    }

    override fun onStart() {
        super.onStart()
        node.startLive() // app open: live on every layer
    }

    override fun onStop() {
        node.stopLive()
        SyncWorker.flushNow(this) // everything is saved; send it without waiting (§8)
        super.onStop()
    }
}
