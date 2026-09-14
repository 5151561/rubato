pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        gradlePluginPortal()
        mavenCentral()
        maven("https://maven.aliyun.com/repository/gradle-plugin")
        maven("https://maven.aliyun.com/repository/public")
    }
}

plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        maven("https://maven.aliyun.com/repository/google")
        maven("https://maven.aliyun.com/repository/public")
        maven("https://jitpack.io") {
            content {
                includeGroupByRegex("com\\.github.*")
            }
        }
        // LegadoTeam 定制构件(htmlunit-core-js -legado 版)
        maven(rootDir.resolve("third_party/maven").toURI())
        mavenCentral()
    }
}

rootProject.name = "rubato-judge"


include(":rhino")
include(":engine")
// 差分 harness(纯 JVM,不属于被冻结的引擎本体)
include(":harness")
// js-host 差分执行器(真 Rhino;与 :harness 的 com.script 桩互斥,故分模块)
include(":jsharness")
// webView 策略差分执行器(挂载**真身** BackstageWebView;与 :harness / :jsharness
// 里那两个 BackstageWebView **确定性桩**同名互斥,故第三个模块)
include(":wvharness")
