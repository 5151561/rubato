@file:Suppress("unused")

package splitties.systemservices

// 差分永远无网络:NetworkUtils.isAvailable 走 activeNetwork == null 分支
val connectivityManager: android.net.ConnectivityManager = android.net.ConnectivityManager()
