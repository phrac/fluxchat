# matrix-rust-sdk's Kotlin bindings call into Rust through JNA, which finds
# classes, fields and callbacks by name: keep them all.
-keep class org.matrix.rustcomponents.** { *; }
-keep class uniffi.** { *; }
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**

# UnifiedPush's service is started by the distributor.
-keep class org.unifiedpush.android.connector.** { *; }
