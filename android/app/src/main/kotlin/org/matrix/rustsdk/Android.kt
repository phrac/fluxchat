package org.matrix.rustsdk

import android.content.Context

/**
 * The SDK's Android hook (`Java_org_matrix_rustsdk_Android_init` in
 * libmatrix_sdk_ffi): hands it the app Context, which rustls-platform-verifier
 * needs to check TLS certificates against Android's trust store. Without it
 * every https request fails ("expect rustls-platform-verifier to be
 * initialized"). The Kotlin side of the verifier is the
 * org.rustls:rustls-platform-verifier dependency.
 */
object Android {
    init {
        System.loadLibrary("matrix_sdk_ffi")
    }

    @JvmStatic
    external fun init(context: Context)
}
