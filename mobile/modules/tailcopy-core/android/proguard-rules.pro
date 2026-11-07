# The uniffi Kotlin bindings call Rust through JNA, which finds structures and
# callbacks by reflection: keep them whole. JNA's desktop helpers refer to AWT,
# which Android doesn't have.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class uniffi.tailcopy_mobile.** { *; }
-dontwarn java.awt.**
