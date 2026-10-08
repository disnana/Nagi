import org.jetbrains.intellij.platform.gradle.TestFrameworkType
import org.jetbrains.intellij.platform.gradle.tasks.VerifyPluginTask

plugins {
    java
    id("org.jetbrains.intellij.platform") version "2.3.0"
}

group = "com.disnana.nagi"
version = "0.1.1"

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
            providers.gradleProperty("platformVersion").getOrElse("2025.1.1"),
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
            else {
                val type = providers.gradleProperty("platformType").getOrElse("IC")
                val version = providers.gradleProperty("platformVersion").getOrElse("2025.1.1")
                ide(type, version, useInstaller = false)
                val minimum = providers.gradleProperty("minimumPlatformVersion").orNull
                if (minimum != null && minimum != version) ide(type, minimum, useInstaller = false)
            }
        }
    }
}

providers.gradleProperty("verificationArchive").orNull?.let { archivePath ->
    tasks.named<VerifyPluginTask>("verifyPlugin") {
        archiveFile.set(file(archivePath))
    }
}

tasks.test { systemProperty("java.awt.headless", "true") }
