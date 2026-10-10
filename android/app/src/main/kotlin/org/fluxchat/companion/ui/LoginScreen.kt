package org.fluxchat.companion.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Dns
import androidx.compose.material.icons.outlined.Key
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material.icons.outlined.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.fluxchat.companion.R
import org.fluxchat.companion.matrix.Matrix

/** The fluxchat mark on its blue tile. */
@Composable
fun Logo(size: androidx.compose.ui.unit.Dp = 88.dp) {
    Box(Modifier.size(size).background(Color(0xFF2F62C8), RoundedCornerShape(size * 0.28f)), contentAlignment = Alignment.Center) {
        Image(painterResource(R.drawable.ic_logo), null, Modifier.size(size * 1.6f))
    }
}

@Composable
private fun Form(title: String, subtitle: String, content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize().safeDrawingPadding().imePadding(), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.widthIn(max = 480.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Spacer(Modifier.height(24.dp))
            Logo()
            Spacer(Modifier.height(4.dp))
            Text(title, style = MaterialTheme.typography.headlineMedium, textAlign = TextAlign.Center)
            Text(subtitle, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
            Spacer(Modifier.height(8.dp))
            content()
        }
    }
}

@Composable
fun LoginScreen() {
    var homeserver by remember { mutableStateOf("") }
    var advanced by remember { mutableStateOf(false) }
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    val error by Matrix.error.collectAsState()
    val scope = rememberCoroutineScope()
    val canSubmit = !busy && user.isNotBlank() && password.isNotEmpty() && (homeserver.isNotBlank() || ':' in user)
    val submit = {
        if (canSubmit) {
            busy = true
            scope.launch {
                Matrix.login(homeserver.ifBlank { user.substringAfter(':', "") }, user, password)
                busy = false
            }
        }
    }
    Form("Your agents, anywhere", "Sign in to your Flux account to follow and answer your agents.") {
        if (advanced) {
            OutlinedTextField(
                homeserver, { homeserver = it }, Modifier.fillMaxWidth(),
                label = { Text("Server address") }, placeholder = { Text("matrix.org") }, // lexicon:ok
                singleLine = true,
                leadingIcon = { Icon(Icons.Outlined.Dns, null) },
                supportingText = { Text("Usually not needed with a full @you:server ID") },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Next),
            )
        }
        OutlinedTextField(
            user, { user = it }, Modifier.fillMaxWidth(),
            label = { Text("Username") }, singleLine = true,
            leadingIcon = { Icon(Icons.Outlined.Person, null) },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Next),
        )
        OutlinedTextField(
            password, { password = it }, Modifier.fillMaxWidth(),
            label = { Text("Password") }, singleLine = true,
            leadingIcon = { Icon(Icons.Outlined.Lock, null) },
            trailingIcon = {
                IconButton(onClick = { showPassword = !showPassword }) {
                    Icon(if (showPassword) Icons.Outlined.VisibilityOff else Icons.Outlined.Visibility, if (showPassword) "Hide password" else "Show password")
                }
            },
            visualTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { submit() }),
        )
        error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium) }
        Button(onClick = { submit() }, enabled = canSubmit, modifier = Modifier.fillMaxWidth().height(52.dp)) {
            if (busy) CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp) else Text("Sign in")
        }
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                "This phone becomes a new session on your account.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f, fill = false),
            )
            TextButton(onClick = { advanced = !advanced }) {
                Text(if (advanced) "Hide server details" else "Sign in on another server")
            }
        }
    }
}

@Composable
fun RecoveryScreen() {
    var key by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val error by Matrix.error.collectAsState()
    val scope = rememberCoroutineScope()
    Form(
        "Unlock your past conversations",
        "Enter your recovery key to read your older conversations here and verify this phone. It's the key fluxchat gave you when you set up backups.",
    ) {
        OutlinedTextField(
            key, { key = it }, Modifier.fillMaxWidth(),
            label = { Text("Recovery key") }, minLines = 2,
            leadingIcon = { Icon(Icons.Outlined.Key, null) },
            visualTransformation = PasswordVisualTransformation(),
        )
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        Button(
            onClick = {
                busy = true
                Matrix.clearError()
                scope.launch {
                    Matrix.recover(key)
                    busy = false
                }
            },
            enabled = !busy && key.isNotBlank(),
            modifier = Modifier.fillMaxWidth().height(52.dp),
        ) { if (busy) CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp) else Text("Unlock") }
        TextButton(onClick = { Matrix.skipRecovery() }) { Text("Not now: skip past conversations") }
    }
}
