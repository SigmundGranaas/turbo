package com.sigmundgranaas.turbo.expressive.feature.auth

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.ui.layout.responsiveContentWidth

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AuthScreen(
    onBack: () -> Unit,
    viewModel: AuthViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val context = LocalContext.current
    val state by viewModel.state.collectAsStateWithLifecycle()
    val form = viewModel.form
    val register = form.mode == AuthMode.Register
    val signedIn = state as? AuthState.SignedIn

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        stringResource(
                            when {
                                signedIn != null -> R.string.auth_account_title
                                register -> R.string.auth_register_title
                                else -> R.string.auth_login_title
                            },
                        ),
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.auth_back))
                    }
                },
            )
        },
    ) { padding ->
        if (signedIn != null) {
            AccountView(email = signedIn.account.email, onSignOut = viewModel::logout, modifier = Modifier.padding(padding))
            return@Scaffold
        }
        Column(
            Modifier
                .padding(padding)
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .responsiveContentWidth(480.dp)
                .padding(horizontal = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Spacer(Modifier.height(12.dp))
            Text(
                stringResource(R.string.auth_tagline),
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(24.dp))

            OutlinedTextField(
                value = form.email,
                onValueChange = viewModel::onEmail,
                label = { Text(stringResource(R.string.auth_email)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Next),
                modifier = Modifier.fillMaxWidth().testTag("authEmail"),
            )
            Spacer(Modifier.height(12.dp))
            OutlinedTextField(
                value = form.password,
                onValueChange = viewModel::onPassword,
                label = { Text(stringResource(R.string.auth_password)) },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = if (register) ImeAction.Next else ImeAction.Done),
                modifier = Modifier.fillMaxWidth().testTag("authPassword"),
            )
            if (register) {
                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = form.confirm,
                    onValueChange = viewModel::onConfirm,
                    label = { Text(stringResource(R.string.auth_confirm)) },
                    singleLine = true,
                    isError = form.confirm.isNotEmpty() && form.confirm != form.password,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
                    modifier = Modifier.fillMaxWidth().testTag("authConfirm"),
                )
            }

            form.error?.let { err ->
                Spacer(Modifier.height(12.dp))
                Text(err, style = MaterialTheme.typography.bodyMedium, color = cs.error, modifier = Modifier.fillMaxWidth().testTag("authError"))
            }

            Spacer(Modifier.height(20.dp))
            Button(
                onClick = viewModel::submit,
                enabled = form.canSubmit,
                modifier = Modifier.fillMaxWidth().height(52.dp).testTag("authSubmit"),
            ) {
                if (form.loading) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.5.dp, color = cs.onPrimary)
                } else {
                    Text(stringResource(if (register) R.string.auth_submit_register else R.string.auth_submit_login))
                }
            }

            Spacer(Modifier.height(8.dp))
            TextButton(onClick = viewModel::toggleMode, modifier = Modifier.testTag("authToggle")) {
                Text(stringResource(if (register) R.string.auth_toggle_to_login else R.string.auth_toggle_to_register))
            }

            Spacer(Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                HorizontalDivider(Modifier.weight(1f))
                Text(stringResource(R.string.auth_or), style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant, modifier = Modifier.padding(horizontal = 12.dp))
                HorizontalDivider(Modifier.weight(1f))
            }
            Spacer(Modifier.height(8.dp))
            OutlinedButton(
                onClick = {
                    viewModel.beginGoogleSignIn { url ->
                        runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) }
                    }
                },
                enabled = !form.loading,
                modifier = Modifier.fillMaxWidth().height(52.dp).testTag("authGoogle"),
            ) {
                Text(stringResource(R.string.auth_google))
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
private fun AccountView(email: String, onSignOut: () -> Unit, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Column(
        modifier
            .fillMaxSize()
            .responsiveContentWidth(480.dp)
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(stringResource(R.string.auth_signed_in_as), style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
        Text(email, style = MaterialTheme.typography.headlineSmall, color = cs.onSurface, modifier = Modifier.testTag("accountEmail"))
        Spacer(Modifier.height(16.dp))
        OutlinedButton(onClick = onSignOut, modifier = Modifier.fillMaxWidth().height(52.dp).testTag("signOut")) {
            Text(stringResource(R.string.auth_sign_out))
        }
    }
}
