const fs = require('fs');

async function commentBenchmarkResults(github, context, regression, firstRun) {
  // Same content as the first JavaScript file above
  let comment = '## 🚀 Performance Benchmark Results\n\n';
  
  if (firstRun) {
    comment += '📊 **First benchmark run** - No baseline for comparison yet\n\n';
    comment += 'This appears to be the first benchmark run. Future PRs will compare against the baseline created after this PR is merged.\n\n';
  } else if (regression) {
    comment += '⚠️ **Performance regression detected!**\n\n';
    comment += 'Some benchmarks show performance degradation compared to the main branch baseline.\n';
    comment += 'Please review the detailed results in the workflow artifacts.\n\n';
    comment += '💡 If this regression is intentional (e.g., trading performance for correctness), please document it in the PR description.\n\n';
  } else {
    comment += '✅ **No significant performance regression detected**\n\n';
    comment += 'Benchmark completed successfully. Performance is within acceptable thresholds.\n\n';
  }
  
  comment += '**📊 Detailed Results:**\n';
  comment += '- Download benchmark artifacts from the workflow run for detailed HTML reports\n';
  comment += '- Artifacts are available for 30 days\n';
  comment += '- Run locally with `./scripts/bench.sh --help` for more options\n\n';
  
  if (regression) {
    comment += '**🔍 Next Steps:**\n';
    comment += '1. Download the benchmark artifacts to see detailed comparisons\n';
    comment += '2. Identify which specific operations are slower\n';
    comment += '3. Consider if the performance trade-off is acceptable\n';
    comment += '4. Optimize critical paths if needed\n';
  }
  
  await github.rest.issues.createComment({
    issue_number: context.issue.number,
    owner: context.repo.owner,
    repo: context.repo.repo,
    body: comment
  });
}

module.exports = { commentBenchmarkResults };