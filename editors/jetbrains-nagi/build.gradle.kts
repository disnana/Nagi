import org.jetbrains.intellij.platform.gradle.TestFrameworkType

plugins {
    java
    id("org.jetbrains.intellij.platform") version "2.3.0"
}

group = "com.disnana.nagi"
version = "0.1.0"

repositories {
    mavenCentral()
    intellijPlatform { defaultRepositories() }
}

dependencies {
    intellijPlatform {
        val localPath = providers.gradleProperty("localPlatformPath").orNull
        if (localPath != null) local(localPath)
        else create(
            providers.gradleProperty("platformType").getOrElse("IC"),
            providers.gradleProperty("platformVersion").getOrElse("2024.3.7"),
            useInstaller = false,
        )
        testFramework(TestFrameworkType.Platform)
        pluginVerifier()
    }
    testImplementation("junit:junit:4.13.2")
}

java {
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
}

intellijPlatform {
    pluginConfiguration {
        ideaVersion {
            sinceBuild = "243"
        }
    }
    pluginVerification {
        ides {
            val localPath = providers.gradleProperty("localPlatformPath").orNull
            if (localPath != null) local(localPath)
            else ide(
                providers.gradleProperty("platformType").getOrElse("IC"),
                providers.gradleProperty("platformVersion").getOrElse("2024.3.7"),
                useInstaller = false,
            )
        }
    }
}

tasks.test { systemProperty("java.awt.headless", "true") }
