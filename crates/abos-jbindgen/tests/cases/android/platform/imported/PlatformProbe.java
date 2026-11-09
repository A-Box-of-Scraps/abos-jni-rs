package com.example;

import android.annotation.NonNull;
import android.annotation.SdkConstant;
import androidx.annotation.RecentlyNullable;
import static android.annotation.SdkConstant.SdkConstantType.ACTIVITY_INTENT_ACTION;
import hidden.implementation.Flags;
import io.github.jni_rs.jbindgen.RustName;
import java.lang.Deprecated;
import java.lang.String;

public class PlatformProbe {
    /** Keeps documentation mentioning @android.annotation.NonNull. */
    @Deprecated
    @RustName("documented_value")
    @NonNull
    @SdkConstant(ACTIVITY_INTENT_ACTION)
    public String value(@RecentlyNullable String documentedKey) {
        throw new RuntimeException("Stub!");
    }
}
